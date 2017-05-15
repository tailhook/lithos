extern crate nix;
extern crate rustc_serialize;
extern crate libc;
extern crate regex;
extern crate argparse;
extern crate quire;
extern crate lithos;
extern crate time;
extern crate fern;
extern crate serde_json;
extern crate syslog;
extern crate signal;
extern crate unshare;
extern crate scan_dir;
#[macro_use] extern crate log;


use std::env;
use std::rc::Rc;
use std::mem::replace;
use std::fs::{File, OpenOptions, metadata, remove_file, rename};
use std::io::{self, stderr, Read, Write};
use std::str::{FromStr};
use std::fs::{remove_dir};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Instant, Duration};
use std::process::exit;
use std::collections::{HashMap, BTreeMap, HashSet};
use std::os::unix::io::RawFd;

use libc::{pid_t, getpid, close};
use libc::{SIGINT, SIGTERM, SIGCHLD};
use nix::sys::signal::{kill, Signal};
use nix::sys::socket::{getsockname, SockAddr};
use nix::sys::socket::{setsockopt, bind, listen};
use nix::sys::socket::{socket, AddressFamily, SockType, SockFlag, InetAddr};
use nix::sys::socket::sockopt::{ReuseAddr, ReusePort};
use quire::{parse_config, Options as COptions};
use regex::Regex;
use serde_json::to_string;
use signal::exec_handler;
use signal::trap::Trap;
use unshare::{Command, reap_zombies, Namespace};

use lithos::cgroup;
use lithos::child_config::ChildConfig;
use lithos::container_config::{ContainerConfig, TcpPort, DEFAULT_KILL_TIMEOUT};
use lithos::container_config::ContainerKind::Daemon;
use lithos::container_config::{InstantiatedConfig, Variables};
use lithos::id_map::IdMapExt;
use lithos::master_config::{MasterConfig, create_master_dirs};
use lithos::MAX_CONFIG_LOGS;
use lithos::sandbox_config::SandboxConfig;
use lithos::setup::{clean_child, init_logging};
use lithos::timer_queue::Queue;
use lithos_tree_options::Options;
use lithos::utils;
use lithos::utils::{clean_dir, relative, ABNORMAL_TERM_SIGNALS};
use lithos::utils::{temporary_change_root};

use self::Timeout::*;

mod lithos_tree_options;

pub const CONFIG_LOG_SIZE: u64 = 10_485_760;

struct Process {
    restart_min: Instant,
    cmd: Command,
    name: String,
    config: Rc<String>,
    inner_config: InstantiatedConfig,
    addresses: Vec<InetAddr>,
    socket_cred: (u32, u32),
}

struct Socket {
    fd: RawFd,
}

enum Child {
    Process(Process),
    Unidentified(String),
}

enum Timeout {
    Start(Process),
    Kill(pid_t),
}

impl Child {
    fn get_name<'x>(&'x self) -> &'x str {
        match self {
            &Child::Process(ref p) => &p.name,
            &Child::Unidentified(ref name) => name,
        }
    }
}


fn new_child(bin: &Binaries, name: &str, master_fn: &Path,
    cfg: &str, options: &Options)
    -> Command
{
    let mut cmd = Command::new(&bin.lithos_knot);
    // Name is first here, so it's easily visible in ps
    cmd.arg("--name");
    cmd.arg(name);
    cmd.arg("--master");
    cmd.arg(master_fn);
    cmd.arg("--config");
    cmd.arg(cfg);
    if options.log_stderr {
        cmd.arg("--log-stderr");
    }
    if let Some(log_level) = options.log_level {
        cmd.arg(format!("--log-level={}", log_level));
    }
    cmd.env_clear();
    cmd.env("TERM", env::var_os("TERM").unwrap_or(From::from("dumb")));
    if let Some(x) = env::var_os("RUST_LOG") {
        cmd.env("RUST_LOG", x);
    }
    if let Some(x) = env::var_os("RUST_BACKTRACE") {
        cmd.env("RUST_BACKTRACE", x);
    }
    cmd.unshare([Namespace::Mount, Namespace::Uts,
                 Namespace::Ipc, Namespace::Pid].iter().cloned());
    cmd
}

fn check_master_config(cfg: &MasterConfig) -> Result<(), String> {
    if metadata(&cfg.devfs_dir).is_err() {
        return Err(format!(
            "Devfs dir ({:?}) must exist and contain device nodes",
            cfg.devfs_dir));
    }
    return Ok(());
}

fn global_init(master: &MasterConfig, options: &Options)
    -> Result<(), String>
{
    try!(create_master_dirs(&master));
    try!(init_logging(&master, &master.log_file, &master.syslog_app_name,
          options.log_stderr,
          options.log_level
            .or_else(|| FromStr::from_str(&master.log_level).ok())
            .unwrap_or(log::LogLevel::Warn)));
    try!(check_process(&master));
    if let Some(ref name) = master.cgroup_name {
        try!(cgroup::ensure_in_group(name, &master.cgroup_controllers));
    }
    return Ok(());
}

fn global_cleanup(master: &MasterConfig) {
    clean_dir(&master.runtime_dir.join(&master.state_dir), false)
        .unwrap_or_else(|e| error!("Error removing state dir: {}", e));
}

fn discard<E>(_: E) { }

fn _read_args(pid: pid_t, global_config: &Path)
    -> Result<(String, String), ()>
{
    let mut buf = String::with_capacity(4096);
    try!(File::open(&format!("/proc/{}/cmdline", pid))
         .and_then(|mut f| f.read_to_string(&mut buf))
         .map_err(discard));
    let args: Vec<&str> = buf[..].splitn(8, '\0').collect();
    if args.len() != 8
       || Path::new(args[0]).file_name()
          .and_then(|x| x.to_str()) != Some("lithos_knot")
       || args[1] != "--name"
       || args[3] != "--master"
       || Path::new(args[4]) != global_config
       || args[5] != "--config"
       || args[7] != ""
    {
       return Err(());
    }
    return Ok((args[2].to_string(), args[6].to_string()));
}

fn _is_child(pid: pid_t, ppid: pid_t) -> bool {
    let mut buf = String::with_capacity(256);
    let ppid_regex = Regex::new(r"^\d+\s+\([^)]*\)\s+\S+\s+(\d+)\s").unwrap();
    if File::open(&format!("/proc/{}/stat", pid))
       .and_then(|mut f| f.read_to_string(&mut buf))
       .is_err() {
        return false;
    }
    return Some(ppid) == ppid_regex.captures(&buf)
         .and_then(|c| FromStr::from_str(c.get(1).unwrap().as_str()).ok());
}


fn check_process(cfg: &MasterConfig) -> Result<(), String> {
    let mypid = unsafe { getpid() };
    let pid_file = cfg.runtime_dir.join("master.pid");
    if metadata(&pid_file).is_ok() {
        let mut buf = String::with_capacity(50);
        match File::open(&pid_file)
            .and_then(|mut f| f.read_to_string(&mut buf))
            .map_err(|_| ())
            .and_then(|_| FromStr::from_str(&buf[..].trim())
                            .map_err(|_| ()))
        {
            Ok::<pid_t, ()>(pid) if pid == mypid => {
                return Ok(());
            }
            Ok(pid) => {
                if kill(pid, None).is_ok() {
                    return Err(format!(concat!("Master pid is {}. ",
                        "And there is alive process with ",
                        "that pid."), pid));

                }
            }
            _ => {
                warn!("Pid file exists, but cannot be read");
            }
        }
    }
    try!(File::create(&pid_file)
        .and_then(|mut f| write!(f, "{}\n", unsafe { getpid() }))
        .map_err(|e| format!("Can't write file {:?}: {}", pid_file, e)));
    return Ok(());
}

fn recover_sockets(sockets: &mut HashMap<InetAddr, Socket>) {
    scan_dir::ScanDir::all().read("/proc/self/fd", |iter| {
        let fds = iter
            .filter_map(|(_, name)| FromStr::from_str(&name).ok())
            .filter(|&x| x >= 3);
        for fd in fds {
            match getsockname(fd) {
                Ok(SockAddr::Inet(addr)) => {
                    let sock = Socket {
                        fd: fd,
                    };
                    match sockets.insert(addr, sock) {
                        None => {}
                        Some(old) => {
                            error!("Addreesss {} has two sockets: \
                                fd={} and fd={}, discarding latter.",
                                addr, fd, old.fd);
                        }
                    }
                }
                Ok(_) => {
                    debug!("Fd {} is different kind of socket", fd);
                }
                Err(_) => {
                    debug!("Fd {} is not a socket", fd);
                }
            }
        }
    }).map_err(|e| error!("Error enumerating my fds: {}", e)).ok();
}

fn recover_processes(children: &mut HashMap<pid_t, Child>,
    configs: &mut HashMap<String, Process>,
    queue: &mut Queue<Timeout>, config_file: &Path)
{
    let mypid = unsafe { getpid() };
    let now = Instant::now();

    // Recover old workers
    scan_dir::ScanDir::all().read("/proc", |iter| {
        let pids = iter.filter_map(|(_, pid)| FromStr::from_str(&pid).ok());
        for pid in pids {
            if !_is_child(pid, mypid) {
                continue;
            }
            if let Ok((name, cfg_text)) = _read_args(pid, config_file) {
                match configs.remove(&name) {
                    Some(child) => {
                        if &child.config[..] != &cfg_text[..] {
                            warn!("Config mismatch: {}, pid: {}. Upgrading...",
                                  name, pid);
                            kill(pid, Signal::SIGTERM)
                            .map_err(|e|
                                error!("Error sending TERM to {}: {:?}",
                                    pid, e)).ok();
                            queue.add(now +
                                    duration(child.inner_config.kill_timeout),
                                Kill(pid));
                        }
                        children.insert(pid, Child::Process(child));
                    }
                    None => {
                        warn!("Undefined child name: {}, pid: {}. \
                            Sending SIGTERM...", name, pid);
                        children.insert(pid, Child::Unidentified(name));
                        kill(pid, Signal::SIGTERM)
                        .map_err(|e| error!("Error sending TERM to {}: {:?}",
                            pid, e)).ok();
                        queue.add(
                            now + duration(DEFAULT_KILL_TIMEOUT),
                            Kill(pid));
                    }
                };
            } else {
                warn!("Undefined child, pid: {}. Sending SIGTERM...", pid);
                kill(pid, Signal::SIGTERM)
                    .map_err(|e| error!("Error sending TERM to {}: {:?}",
                        pid, e)).ok();
                queue.add(
                    now + duration(DEFAULT_KILL_TIMEOUT),
                    Kill(pid));
                continue;
            }
        }
    }).map_err(|e| error!("Error reading /proc: {}", e)).ok();
}

fn remove_dangling_state_dirs(names: &HashSet<String>, master: &MasterConfig)
{
    let pid_regex = Regex::new(r"\.(\d+)$").unwrap();
    let master = master.runtime_dir.join(&master.state_dir);
    scan_dir::ScanDir::dirs().read(&master, |iter| {
        for (entry, sandbox_name) in iter {
            let path = entry.path();
            debug!("Checking sandbox dir: {:?}", path);
            let mut valid_dirs = 0;
            scan_dir::ScanDir::dirs().read(&path, |iter| {
                for (entry, proc_name) in iter {
                    let name = format!("{}/{}", sandbox_name, proc_name);
                    debug!("Checking process dir: {}", name);
                    if names.contains(&name) {
                        valid_dirs += 1;
                        continue;
                    } else if proc_name.starts_with("cmd.") {
                        debug!("Checking command dir: {}", name);
                        let pid = pid_regex.captures(&proc_name).and_then(
                            |c| FromStr::from_str(c.get(1).unwrap().as_str()).ok());
                        if let Some(pid) = pid {
                            if kill(pid, None).is_ok() {
                                valid_dirs += 1;
                                continue;
                            }
                        }
                    }
                    let path = entry.path();
                    warn!("Dangling state dir {:?}. Deleting...", path);
                    clean_dir(&path, true)
                        .map_err(|e| error!(
                            "Can't remove dangling state dir {:?}: {}",
                            path, e))
                        .ok();
                }
            }).map_err(|e|
                error!("Error reading state dir {:?}: {}", path, e)).ok();
            debug!("Tree dir {:?} has {} valid subdirs", path, valid_dirs);
            if valid_dirs > 0 {
                continue;
            }
            warn!("Empty sandbox dir {:?}. Deleting...", path);
            clean_dir(&path, true)
                .map_err(|e| error!("Can't empty state dir {:?}: {}", path, e))
                .ok();
        }
    }).map_err(|e| error!("Error listing state dir: {}", e)).ok();
}

fn _rm_cgroup(dir: &Path) {
    if let Err(e) = remove_dir(dir) {
        let mut buf = String::with_capacity(1024);
        File::open(&dir.join("cgroup.procs"))
            .and_then(|mut f| f.read_to_string(&mut buf))
            .ok();
        error!("Error removing cgroup: {} (processes {:?})",
            e, buf);
    }
}

fn remove_dangling_cgroups(names: &HashSet<String>, master: &MasterConfig)
{
    if master.cgroup_name.is_none() {
        return;
    }
    let cgroups = match cgroup::parse_cgroups(None) {
        Ok(cgroups) => cgroups,
        Err(e) => {
            error!("Can't parse my cgroups: {}", e);
            return;
        }
    };
    // TODO(tailhook) need to customize cgroup mount point?
    let cgroup_base = Path::new("/sys/fs/cgroup");
    let root_path = Path::new("/");
    let child_group_regex = Regex::new(r"^([\w-]+):([\w-]+\.\d+)\.scope$")
        .unwrap();
    let cmd_group_regex = Regex::new(r"^([\w-]+):cmd\.[\w-]+\.(\d+)\.scope$")
        .unwrap();
    let cgroup_filename = master.cgroup_name.as_ref().map(|x| &x[..]);

    // Loop over all controllers in case someone have changed config
    for cgrp in cgroups.all_groups.iter() {
        let cgroup::CGroupPath(ref folder, ref path) = **cgrp;
        let ctr_dir = cgroup_base.join(&folder).join(
            &relative(path, &root_path));
        if path.file_name().and_then(|x| x.to_str()) == cgroup_filename {
            debug!("Checking controller dir: {:?}", ctr_dir);
        } else {
            debug!("Skipping controller dir: {:?}", ctr_dir);
            continue;
        }
        scan_dir::ScanDir::dirs().read(&ctr_dir, |iter| {
            for (entry, filename) in iter {
                if let Some(capt) = child_group_regex.captures(&filename) {
                    let name = format!("{}/{}",
                        capt.get(1).unwrap().as_str(),
                        capt.get(2).unwrap().as_str());
                    if !names.contains(&name) {
                        _rm_cgroup(&entry.path());
                    }
                } else if let Some(capt) = cmd_group_regex.captures(&filename)
                {
                    let pid = FromStr::from_str(
                        capt.get(2).unwrap().as_str()).ok();
                    if pid.is_none() || !kill(pid.unwrap(), None).is_ok() {
                        _rm_cgroup(&entry.path());
                    }
                } else {
                    warn!("Skipping wrong group {:?}", entry.path());
                    continue;
                }
            }
        }).map_err(|e| error!("Error reading cgroup dir {:?}: {}",
            ctr_dir, e)).ok();
    }
}

fn run(config_file: &Path, options: &Options)
    -> Result<(), String>
{
    let master: MasterConfig = try!(parse_config(&config_file,
        &MasterConfig::validator(), &COptions::default())
        .map_err(|e| format!("Error reading master config: {}", e)));
    try!(check_master_config(&master));
    try!(global_init(&master, &options));

    let bin = match get_binaries() {
        Some(bin) => bin,
        None => {
            exit(127);
        }
    };

    let mut trap = Trap::trap(&[SIGINT, SIGTERM, SIGCHLD]);
    let config_file = config_file.to_owned();

    let mut configs = read_sandboxes(&master, &bin, &config_file, options);

    info!("Recovering Sockets");
    let mut queue = Queue::new();
    let mut sockets = HashMap::new();
    recover_sockets(&mut sockets);
    info!("Recovering Processes");
    let mut children = HashMap::new();
    recover_processes(&mut children, &mut configs, &mut queue, &config_file);
    close_unused_sockets(&mut sockets, &mut children);

    let recovered = children.values()
        .map(|c| c.get_name().to_string()).collect();

    info!("Removing Dangling State Dirs");
    remove_dangling_state_dirs(&recovered, &master);

    info!("Removing Dangling CGroups");
    remove_dangling_cgroups(&recovered, &master);

    info!("Starting Processes");
    schedule_new_workers(configs, &mut queue);

    normal_loop(&mut queue, &mut children, &mut sockets, &mut trap, &master);
    if children.len() > 0 {
        shutdown_loop(&mut children, &mut sockets, &mut trap, &master);
    }

    global_cleanup(&master);

    return Ok(());
}

fn close_unused_sockets(sockets: &mut HashMap<InetAddr, Socket>,
                        children: &HashMap<pid_t, Child>)
{
    let empty = Vec::new();
    let used_addresses: HashSet<InetAddr> = children.values().flat_map(|ch| {
        match ch {
            &Child::Process(ref p) => p.addresses.iter().cloned(),
            &Child::Unidentified(_) => empty.iter().cloned(),
        }
    }).collect();
    *sockets = replace(sockets, HashMap::new())
        .into_iter().filter(|&(p, ref s)| {
            if used_addresses.contains(&p) {
                true
            } else {
                unsafe { close(s.fd) };
                false
            }
        }).collect();
}

fn open_socket(addr: InetAddr, cfg: &TcpPort, uid: u32, gid: u32)
    -> Result<RawFd, String>
{

    let sock = {
        let _fsuid_guard = utils::FsUidGuard::set(uid, gid);
        try!(socket(AddressFamily::Inet,
            SockType::Stream, SockFlag::empty(), 0)
            .map_err(|e| format!("Can't create socket: {:?}", e)))
    };

    let mut result = Ok(());
    if cfg.reuse_addr {
        result = result.and_then(|_| setsockopt(sock, ReuseAddr, &true));
    }
    if cfg.reuse_port {
        result = result.and_then(|_| setsockopt(sock, ReusePort, &true));
    }
    result =  result.and_then(|_| bind(sock, &SockAddr::Inet(addr)));
    result =  result.and_then(|_| listen(sock, cfg.listen_backlog));
    if let Err(e) = result {
        unsafe { close(sock) };
        Err(format!("Socket option error: {:?}", e))
    } else {
        Ok(sock)
    }
}

fn open_sockets_for(socks: &mut HashMap<InetAddr, Socket>,
                    ports: &HashMap<u16, TcpPort>,
                    cmd: &mut Command,
                    uid: u32, gid: u32)
    -> Result<(), String>
{
    for (&port, item) in ports {
        let addr = InetAddr::from_std(&SocketAddr::new(item.host.0, port));
        if !socks.contains_key(&addr) {
            if !item.reuse_port {
                let sock = try!(open_socket(addr, item, uid, gid));
                socks.insert(addr, Socket {
                    fd: sock,
                });
            }
        }
    }

    cmd.reset_fds();
    if socks.len() > 0 {
        cmd.close_fds(socks.values().map(|x| x.fd).min().unwrap()
                      ..(socks.values().map(|x| x.fd).max().unwrap() + 1));
        for (&port, item) in ports {
            let addr = InetAddr::from_std(&SocketAddr::new(item.host.0, port));
            unsafe {
                cmd.file_descriptor_raw(
                    item.fd,
                    socks.get(&addr).unwrap().fd);
            }
        }
    }
    Ok(())
}

fn duration(inp: f32) -> Duration {
    Duration::from_millis((inp * 1000.) as u64)
}

fn normal_loop(queue: &mut Queue<Timeout>,
    children: &mut HashMap<pid_t, Child>,
    sockets: &mut HashMap<InetAddr, Socket>,
    trap: &mut Trap, master: &MasterConfig)
{
    loop {
        let now = Instant::now();

        let mut buf = Vec::new();
        for timeout in queue.pop_until(now) {
            match timeout {
                Start(mut child) => {
                    let restart_min = now +
                        duration(child.inner_config.restart_timeout);
                    match open_sockets_for(
                        sockets, &child.inner_config.tcp_ports,
                        &mut child.cmd,
                        child.socket_cred.0, child.socket_cred.1)
                    {
                        Ok(()) => {}
                        Err(e) => {
                            error!("Error starting {:?}, \
                                error opening sockets: {}",
                                child.name, e);
                            buf.push((restart_min, child));
                            continue;
                        }
                    }
                    match child.cmd.spawn() {
                        Ok(c) => {
                            child.restart_min = restart_min;
                            children.insert(c.pid(), Child::Process(child));
                        }
                        Err(e) => {
                            error!("Error starting {:?}: {}", child.name, e);
                            buf.push((restart_min, child));
                        }
                    }
                }
                Kill(pid) => {
                    if children.contains_key(&pid) {  // if not already dead
                        error!("Process {:?} looks like hanging. \
                            Sending kill...",
                            pid);
                        kill(pid, Signal::SIGKILL).ok();
                    }
                }
            }
        }
        for (restart_min, v) in buf.into_iter() {
            queue.add(restart_min, Start(v));
        }

        close_unused_sockets(sockets, children);
        let next_signal = match queue.peek_time() {
            Some(deadline) => trap.wait(deadline),
            None => trap.next(),
        };
        match next_signal {
            None => {
                continue;
            }
            Some(SIGINT) => {
                // SIGINT is usually a Ctrl+C so it's sent to whole
                // process group, so we don't need to do anything special
                debug!("Received SIGINT. Waiting process to stop..");
                return;
            }
            Some(SIGTERM) => {
                // SIGTERM is usually sent to a specific process so we
                // forward it to children
                debug!("Received SIGTERM signal, propagating");
                for (&pid, _) in children {
                    kill(pid, Signal::SIGTERM).ok();
                }
                return;
            }
            Some(SIGCHLD) => {
                for (pid, status) in reap_zombies() {
                    match children.remove(&pid) {
                        Some(Child::Process(child)) => {
                            error!("Process {:?} {}", child.name, status);
                            clean_child(&child.name, &master, true);
                            queue.add(child.restart_min, Start(child));
                        }
                        Some(Child::Unidentified(name)) => {
                            clean_child(&name, &master, false);
                        }
                        None => {
                            info!("Unknown process {:?} {}", pid, status);
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

fn shutdown_loop(children: &mut HashMap<pid_t, Child>,
    sockets: &mut HashMap<InetAddr, Socket>,
    trap: &mut Trap,
    master: &MasterConfig)
{
    for sig in trap {
        match sig {
            SIGINT => {
                // SIGINT is usually a Ctrl+C so it's sent to whole
                // process group, so we don't need to do anything special
                debug!("Received SIGINT. Waiting process to stop..");
                continue;
            }
            SIGTERM => {
                // SIGTERM is usually sent to a specific process so we
                // forward it to children
                debug!("Received SIGTERM signal, propagating");
                for &pid in children.keys() {
                    kill(pid, Signal::SIGTERM).ok();
                }
                continue;
            }
            SIGCHLD => {
                for (pid, status) in reap_zombies() {
                    match children.remove(&pid) {
                        Some(child) => {
                            info!("Process {:?} {}", child.get_name(), status);
                            clean_child(child.get_name(), &master, false);
                        }
                        None => {
                            info!("Unknown process {:?} {}", pid, status);
                        }
                    }
                }
                // In case we will wait for some process for the long time
                // we want to close tcp ports as fast as possible, so that
                // our upstream/monitoring notice the socket is closed
                close_unused_sockets(sockets, children);
                if children.len() == 0 {
                    return;
                }
            }
            _ => unreachable!(),
        }
    }
}

fn read_sandboxes(master: &MasterConfig, bin: &Binaries,
    master_file: &Path, options: &Options)
    -> HashMap<String, Process>
{
    let dirpath = master_file.parent().unwrap().join(&master.sandboxes_dir);
    info!("Reading sandboxes from {:?}", dirpath);
    let sandbox_validator = SandboxConfig::validator();
    scan_dir::ScanDir::files().read(&dirpath, |iter| {
        let yamls = iter.filter(|&(_, ref name)| name.ends_with(".yaml"));
        yamls.filter_map(|(entry, name)| {
            let sandbox_config = entry.path();
            let sandbox_name = name[..name.len()-5].to_string();
            debug!("Reading config: {:?}", sandbox_config);
            parse_config(&sandbox_config, &sandbox_validator, &COptions::default())
                .map_err(|e| error!("Can't read config {:?}: {}",
                                    sandbox_config, e))
                .map(|cfg: SandboxConfig| (sandbox_name, cfg))
                .ok()
        }).flat_map(|(name, sandbox)| {
            read_subtree(master, bin, master_file, &name, &sandbox, options)
            .into_iter()
        }).collect()
    })
    .map_err(|e| error!("Error reading sandboxes directory: {}", e))
    .unwrap_or(HashMap::new())
}

fn open_config_log(base: &Path, name: &str) -> Result<File, io::Error> {
    let target_name = base.join(name);
    let file = OpenOptions::new().create(true).write(true).append(true)
        .open(&target_name)?;
    let logmeta = file.metadata()?;
    if logmeta.len() > CONFIG_LOG_SIZE {
        let fname = base.join(format!("{}.{}", name, MAX_CONFIG_LOGS));
        match remove_file(&fname) {
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                error!("Can't remove log file {:?}: {}", fname, e);
            }
            Ok(()) => {
                debug!("Removed {:?}", fname);
            }
        };
        let mut prevname = fname.clone();
        for i in (MAX_CONFIG_LOGS-1)..0 {
            let fname = base.join(format!("{}.{}", name, i));
            match rename(&fname, &prevname) {
                Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => {
                    error!("Can't rename log file {:?}: {}", fname, e);
                }
                Ok(()) => {
                    debug!("Renamed {:?}", fname);
                }
            };
            prevname = fname;
        }
        match rename(&fname, &prevname) {
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                error!("Can't rename log file {:?}: {}", fname, e);
            }
            Ok(()) => {
                debug!("Renamed {:?}", fname);
            }
        };
        // reopen same path
        OpenOptions::new().create(true).write(true).append(true)
           .open(base.join(name))
    } else {
        Ok(file)
    }
}

fn read_subtree<'x>(master: &MasterConfig,
    bin: &Binaries, master_file: &Path,
    sandbox_name: &String, sandbox: &SandboxConfig,
    options: &Options)
    -> Vec<(String, Process)>
{
    let now = Instant::now();
    let cfg = master_file.parent().unwrap()
        .join(&master.processes_dir)
        .join(sandbox.config_file.as_ref().map(Path::new)
            .unwrap_or(Path::new(&(sandbox_name.clone() + ".yaml"))));
    debug!("Reading child config {:?}", cfg);
    parse_config(&cfg, &ChildConfig::mapping_validator(), &COptions::default())
        .map(|cfg: BTreeMap<String, ChildConfig>| {
            open_config_log(
                &master.config_log_dir,
                &format!("{}.log", sandbox_name)
            ).and_then(|mut f| {
                // we want as atomic writes as possible, so format into a buf
                let buf = format!("{} {}\n",
                    time::now_utc().rfc3339(),
                    to_string(&cfg).unwrap());
                f.write_all(buf.as_bytes())
            })
            .map_err(|e| error!("Error writing config log: {}", e))
            .ok();
            cfg
        })
        .map_err(|e| warn!("Can't read config {:?}: {}", cfg, e))
        .unwrap_or(BTreeMap::new())
        .into_iter()
        .filter(|&(_, ref child)| child.kind == Daemon)
        .flat_map(|(child_name, mut child)| {
            let instances = child.instances;

            //  Child doesn't need to know how many instances it's run
            //  And for comparison on restart we need to have "one" always
            child.instances = 1;
            let image_dir = sandbox.image_dir.join(&child.image);
            let cfg_res = temporary_change_root(&image_dir, || {
                parse_config(&child.config,
                    &ContainerConfig::validator(), &COptions::default())
                .map_err(|e| format!("Error reading {:?} \
                    of sandbox {:?} of image {:?}: {}",
                    &child.config, sandbox_name, child.image,  e))
            });
            let cfg: ContainerConfig = match cfg_res {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!("{}", e);
                    return Vec::new().into_iter();
                }
            };
            let child_string = Rc::new(to_string(&child).unwrap());
            let mut sock_uid = cfg.user_id;
            let mut sock_gid = cfg.group_id;
            if sandbox.uid_map.len() > 0 {
                sock_uid = sandbox.uid_map.map_id(sock_uid).unwrap_or(0);
                sock_gid = sandbox.gid_map.map_id(sock_gid).unwrap_or(0);
            } else if cfg.uid_map.len() > 0 {
                sock_uid = cfg.uid_map.map_id(sock_uid).unwrap_or(0);
                sock_gid = cfg.gid_map.map_id(sock_gid).unwrap_or(0);
            }

            let mut items = Vec::<(String, Process)>::new();
            for i in 0..instances {
                let name = format!("{}/{}.{}", sandbox_name, child_name, i);
                let cfg = match cfg.instantiate(&Variables {
                        user_vars: &child.variables,
                        lithos_name: &name,
                        lithos_config_filename: &child.config,
                    }) {
                    Ok(x) => x,
                    Err(e) => {
                        error!("Variable substitution error {:?} \
                            of sandbox {:?} of image {:?}: {}",
                            &child.config, sandbox_name, child.image,
                            e.join("; "));
                        continue;
                    }
                };
                let cmd = new_child(bin, &name, master_file,
                    &child_string, options);
                let restart_min = now + duration(cfg.restart_timeout);
                let process = Process {
                    cmd: cmd,
                    name: name.clone(),
                    restart_min: restart_min,
                    config: child_string.clone(), // should avoid cloning?
                    addresses: cfg.tcp_ports.iter().map(|(&port, item)| {
                            InetAddr::from_std(
                                &SocketAddr::new(item.host.0, port))
                        }).collect(),
                    inner_config: cfg,
                    socket_cred: (sock_uid, sock_gid),
                };
                items.push((name, process));
            }
            items.into_iter()
        }).collect()
}

fn schedule_new_workers(configs: HashMap<String, Process>,
    queue: &mut Queue<Timeout>)
{
    for (_, item) in configs.into_iter() {
        queue.add(Instant::now(), Start(item));
    }
}

struct Binaries {
    lithos_tree: PathBuf,
    lithos_knot: PathBuf,
}

fn get_binaries() -> Option<Binaries> {
    let dir = match env::current_exe().ok()
        .and_then(|x| x.parent().map(|y| y.to_path_buf()))
    {
        Some(dir) => dir,
        None => return None,
    };
    let bin = Binaries {
        lithos_tree: dir.join("lithos_tree"),
        lithos_knot: dir.join("lithos_knot"),
    };
    if !metadata(&bin.lithos_tree).map(|x| x.is_file()).unwrap_or(false) {
        write!(&mut stderr(), "Can't find lithos_tree binary").unwrap();
        return None;
    }
    if !metadata(&bin.lithos_knot).map(|x| x.is_file()).unwrap_or(false) {
        write!(&mut stderr(), "Can't find lithos_knot binary").unwrap();
        return None;
    }
    return Some(bin);
}

fn main() {
    exec_handler::set_handler(&ABNORMAL_TERM_SIGNALS, true)
        .ok().expect("Can't set singal handler");

    let options = match Options::parse_args() {
        Ok(options) => options,
        Err(x) => {
            exit(x);
        }
    };
    match run(&options.config_file, &options) {
        Ok(()) => {
            exit(0);
        }
        Err(e) => {
            write!(&mut stderr(), "Fatal error: {}\n", e).ok();
            error!("Fatal error: {}", e);
            exit(1);
        }
    }
}
