extern crate rustc_serialize;
extern crate libc;
#[macro_use] extern crate log;
extern crate regex;
extern crate argparse;
extern crate quire;
extern crate lithos;
extern crate time;
extern crate fern;


use std::env;
use std::rc::Rc;
use std::io::Error as IoError;
use std::fs::{File, metadata};
use std::io::{stderr, Read, Write};
use std::str::{FromStr};
use std::fs::{read_dir, remove_dir};
use std::ptr::null;
use std::path::{Path, PathBuf};
use std::ffi::{CString};
use std::default::Default;
use std::process::exit;
use std::collections::HashMap;
use std::collections::BTreeMap;

use libc::pid_t;
use libc::funcs::posix88::unistd::{getpid, execv};
use regex::Regex;
use quire::parse_config;
use rustc_serialize::json;

use lithos::setup::{clean_child, init_logging};
use lithos::master_config::{MasterConfig, create_master_dirs};
use lithos::tree_config::TreeConfig;
use lithos::child_config::ChildConfig;
use lithos::container_config::ContainerKind::Daemon;
use lithos::monitor::{Monitor, Executor};
use lithos::monitor::MonitorResult::{Killed, Reboot};
use lithos::container::Command;
use lithos::utils::{clean_dir, get_time, relative, cpath};
use lithos::signal;
use lithos::cgroup;
use lithos_tree_options::Options;

mod lithos_tree_options;


struct Child {
    name: Rc<String>,
    master_file: Rc<PathBuf>,
    child_config_serialized: Rc<String>,
    master_config: Rc<MasterConfig>,
    child_config: Rc<ChildConfig>,
    root_binary: Rc<PathBuf>,
    log_level: log::LogLevel,
    log_stderr: bool,
}

impl Executor for Child {
    fn command(&self) -> Command
    {
        let mut cmd = Command::new((*self.name).clone(), &*self.root_binary);
        cmd.keep_sigmask();

        // Name is first here, so it's easily visible in ps
        cmd.arg("--name");
        cmd.arg(&self.name[..]);

        cmd.arg("--master");
        cmd.arg(&self.master_file.to_str().unwrap()[..]);
        cmd.arg("--config");
        cmd.arg(&self.child_config_serialized[..]);
        if self.log_stderr {
            cmd.arg("--log-stderr");
        }
        cmd.arg(format!("--log-level={}", self.log_level));
        cmd.set_env("TERM".to_string(),
                    env::var("TERM").unwrap_or("dumb".to_string()));
        if let Ok(x) = env::var("RUST_LOG") {
            cmd.set_env("RUST_LOG".to_string(), x);
        }
        if let Ok(x) = env::var("RUST_BACKTRACE") {
            cmd.set_env("RUST_BACKTRACE".to_string(), x);
        }
        cmd.container();
        return cmd;
    }
    fn finish(&self) -> bool {
        clean_child(&*self.name, &*self.master_config);
        return true;
    }
}

struct UnidentifiedChild {
    name: Rc<String>,
    master_config: Rc<MasterConfig>,
}

impl Executor for UnidentifiedChild {
    fn command(&self) -> Command {
        unreachable!();
    }
    fn finish(&self) -> bool {
        clean_child(&*self.name, &*self.master_config);
        return false;
    }
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
    try!(create_master_dirs(&*master));
    try!(init_logging(&master.default_log_dir.join(&master.log_file),
                      options.log_level, options.log_stderr));
    try!(check_process(&*master));
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
                     .and_then(|c| FromStr::from_str(c.at(1).unwrap()).ok());
}


fn check_process(cfg: &MasterConfig) -> Result<(), String> {
    let mypid = unsafe { getpid() };
    let pid_file = cfg.runtime_dir.join("master.pid");
    if metadata(&pid_file).is_ok() {
        let mut buf = String::with_capacity(50);
        match File::open(&pid_file)
            .and_then(|mut f| f.read_to_string(&mut buf))
            .map_err(|_| ())
            .and_then(|_| FromStr::from_str(&buf[..])
                            .map_err(|_| ()))
        {
            Ok::<pid_t, ()>(pid) if pid == mypid => {
                return Ok(());
            }
            Ok(pid) => {
                if signal::is_process_alive(pid) {
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

fn recover_processes(master: &Rc<MasterConfig>, mon: &mut Monitor,
    configs: &mut HashMap<Rc<String>, Child>, config_file: &Rc<PathBuf>)
{
    let mypid = unsafe { getpid() };

    // Recover old workers
    for pid in read_dir(&"/proc")
        .map_err(|e| format!("Can't read procfs: {}", e))
        .map(|x| x.collect())
        .unwrap_or(Vec::new())
        .into_iter()
        .filter_map(|p| p.ok())
        .filter_map(|p| p.path().file_name()
            .and_then(|x| x.to_str())
            .and_then(|x| FromStr::from_str(x).ok()))
    {
        if !_is_child(pid, mypid) {
            continue;
        }
        if let Ok((name, cfg_text)) = _read_args(pid, &**config_file) {
            let cfg = json::decode(&cfg_text)
                .map_err(|e| warn!(
                    "Error parsing recover config, pid {}, name {:?}: {:?}",
                    pid, name, e))
                .ok();
            let name = Rc::new(name);
            match configs.remove(&name) {
                Some(child) => {
                    if Some(&*child.child_config) != cfg.as_ref() {
                        warn!("Config mismatch: {}, pid: {}. Upgrading...",
                              name, pid);
                        signal::send_signal(pid, signal::SIGTERM);
                    }
                    mon.add(name.clone(), Box::new(child), 1000,
                        Some((pid, get_time())));
                }
                None => {
                    warn!("Undefined child name: {}, pid: {}. Sending SIGTERM...",
                          name, pid);
                    mon.add(name.clone(), Box::new(UnidentifiedChild {
                        name: name,
                        master_config: master.clone(),
                        }), 0,
                        Some((pid, get_time())));
                    signal::send_signal(pid, signal::SIGTERM);
                }
            };
        } else {
            warn!("Undefined child, pid: {}. Sending SIGTERM...",
                  pid);
            signal::send_signal(pid, signal::SIGTERM);
            continue;
        }
    }
}

fn remove_dangling_state_dirs(mon: &Monitor, master: &MasterConfig) {
    let pid_regex = Regex::new(r"\.\(\d+\)$").unwrap();
    for entry in read_dir(&master.runtime_dir.join(&master.state_dir))
        .map_err(|e| error!("Can't read state dir: {}", e))
        .map(|x| x.collect())
        .unwrap_or(Vec::new())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        debug!("Checking tree dir: {:?}", entry.path());
        let mut valid_dirs = 0usize;
        if let Some(tree_name) = entry.path().file_name()
                                .and_then(|x| x.to_str())
        {
            for cont in read_dir(entry.path())
                .map_err(|e| format!("Can't read state dir: {}", e))
                .map(|x| x.collect())
                .unwrap_or(Vec::new())
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if let Some(proc_name) = cont.path().file_name()
                                        .and_then(|x| x.to_str())
                {
                    let name = Rc::new(format!("{}/{}", tree_name, proc_name));
                    debug!("Checking process dir: {}", name);
                    if mon.has(&name) {
                        valid_dirs += 1;
                        continue;
                    } else if proc_name.starts_with("cmd.") {
                        debug!("Checking command dir: {}", name);
                        let pid = pid_regex.captures(proc_name).and_then(
                            |c| FromStr::from_str(c.at(1).unwrap()).ok());
                        if let Some(pid) = pid {
                            if signal::is_process_alive(pid) {
                                valid_dirs += 1;
                                continue;
                            }
                        }
                    }
                }
                warn!("Dangling state dir {:?}. Deleting...", cont.path());
                clean_dir(&cont.path(), true)
                    .map_err(|e| error!(
                        "Can't remove dangling state dir {:?}: {}",
                        cont.path(), e))
                    .ok();
            }
        }
        debug!("Tree dir {:?} has {} valid subdirs", entry.path(), valid_dirs);
        if valid_dirs > 0 {
            continue;
        }
        warn!("Empty tree dir {:?}. Deleting...", entry.path());
        clean_dir(&entry.path(), true)
            .map_err(|e| error!("Can't empty state dir {:?}: {}",
                entry.path(), e))
            .ok();
    }
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

fn remove_dangling_cgroups(mon: &Monitor, master: &MasterConfig) {
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
        for child_dir in read_dir(&ctr_dir)
            .map_err(|e| debug!("Can't read controller {:?} dir: {}",
                                ctr_dir, e))
            .map(|x| x.collect())
            .unwrap_or(Vec::new())
            .into_iter()
            .filter_map(|x| x.ok())
        {
            if !metadata(child_dir.path())
                         .map(|x| x.is_dir()).unwrap_or(false)
            {
                continue;
            }
            let filename = child_dir.path().file_name()
                           .and_then(|x| x.to_str())
                           .map(|x| x.to_string());
            if filename.is_none() {
                warn!("Wrong filename in cgroup: {:?}", child_dir.path());
                continue;
            }
            let filename = filename.unwrap();
            if let Some(capt) = child_group_regex.captures(&filename) {
                let name = format!("{}/{}",
                    capt.at(1).unwrap(), capt.at(2).unwrap());
                if !mon.has(&Rc::new(name)) {
                    _rm_cgroup(&child_dir.path());
                }
            } else if let Some(capt) = cmd_group_regex.captures(&filename) {
                let pid = FromStr::from_str(capt.at(2).unwrap()).ok();
                if pid.is_none() || !signal::is_process_alive(pid.unwrap()) {
                    _rm_cgroup(&child_dir.path());
                }
            } else {
                warn!("Skipping wrong group {:?}", child_dir.path());
                continue;
            }
        }
    }
}

fn run(config_file: &Path, options: &Options)
    -> Result<(), String>
{
    let master: Rc<MasterConfig> = Rc::new(try!(parse_config(&config_file,
        &*MasterConfig::validator(), Default::default())
        .map_err(|e| format!("Error reading master config: {}", e))));
    try!(check_master_config(&*master));
    try!(global_init(&*master, &options));

    let bin = match get_binaries() {
        Some(bin) => bin,
        None => {
            exit(127);
        }
    };

    let config_file = Rc::new(config_file.to_owned());
    let mut mon = Monitor::new("lithos-tree".to_string());

    info!("Reading tree configs from {:?}", master.config_dir);
    let mut configs = read_configs(&master, &bin, &config_file, options);

    info!("Recovering Processes");
    recover_processes(&master, &mut mon, &mut configs, &config_file);

    info!("Removing Dangling State Dirs");
    remove_dangling_state_dirs(&mon, &*master);

    info!("Removing Dangling CGroups");
    remove_dangling_cgroups(&mon, &*master);

    info!("Starting Processes");
    schedule_new_workers(&mut mon, configs);

    mon.allow_reboot();
    match mon.run() {
        Killed => {}
        Reboot => {
            reexec_myself(&bin.lithos_tree);
        }
    }

    global_cleanup(&*master);

    return Ok(());
}

fn read_configs(master: &Rc<MasterConfig>, bin: &Binaries,
    master_file: &Rc<PathBuf>, options: &Options)
    -> HashMap<Rc<String>, Child>
{
    let tree_validator = TreeConfig::validator();
    let name_re = Regex::new(r"^([\w-]+)\.yaml$").unwrap();
    read_dir(&master.config_dir)
        .map_err(|e| { error!("Can't read config dir: {}", e); e })
        .map(|x| x.collect())
        .unwrap_or(Vec::new())
        .into_iter()
        .filter_map(|f| f.ok())
        .filter_map(|f| {
            let name = match f.path().file_name()
                            .and_then(|f| f.to_str())
                            .and_then(|s| name_re.captures(s))
            {
                Some(capt) => capt.at(1).unwrap().to_string(),
                None => return None,
            };
            debug!("Reading config: {:?}", f.path());
            parse_config(&f.path(), &*tree_validator, Default::default())
                .map_err(|e| warn!("Can't read config {:?}: {}", f.path(), e))
                .map(|cfg: TreeConfig| (name.to_string(), cfg))
                .ok()
        })
        .flat_map(|(name, tree)| {
            read_subtree(master, bin, master_file, &name,
                         Rc::new(tree), options)
            .into_iter()
        })
        .collect()
}

fn read_subtree<'x>(master: &Rc<MasterConfig>,
    bin: &Binaries, master_file: &Rc<PathBuf>,
    tree_name: &String, tree: Rc<TreeConfig>,
    options: &Options)
    -> Vec<(Rc<String>, Child)>
{
    debug!("Reading child config {:?}", tree.config_file);
    parse_config(&tree.config_file,
        &*ChildConfig::mapping_validator(), Default::default())
        .map_err(|e| warn!("Can't read config {:?}: {}", tree.config_file, e))
        .unwrap_or(BTreeMap::<String, ChildConfig>::new())
        .into_iter()
        .filter(|&(_, ref child)| child.kind == Daemon)
        .flat_map(|(child_name, mut child)| {
            let instances = child.instances;

            //  Child doesn't need to know how many instances it's run
            //  And for comparison on restart we need to have "one" always
            child.instances = 1;
            let child_string = Rc::new(json::encode(&child).unwrap());

            let child = Rc::new(child);
            let items: Vec<(Rc<String>, Child)> = (0..instances)
                .map(|i| {
                    let name = format!("{}/{}.{}", tree_name, child_name, i);
                    let name = Rc::new(name);
                    (name.clone(), Child {
                        name: name,
                        master_file: master_file.clone(),
                        child_config_serialized: child_string.clone(),
                        master_config: master.clone(),
                        child_config: child.clone(),
                        root_binary: bin.lithos_knot.clone(),
                        log_stderr: options.log_stderr,
                        log_level: options.log_level,
                    })
                })
                .collect();
            items.into_iter()
        }).collect()
}

fn schedule_new_workers(mon: &mut Monitor,
    children: HashMap<Rc<String>, Child>)
{
    for (name, child) in children.into_iter() {
        if mon.has(&name) {
            continue;
        }
        mon.add(name.clone(), Box::new(child), 2000, None);
    }
}

fn reexec_myself(lithos_tree: &Path) -> ! {
    let c_exe = cpath(lithos_tree);
    let c_args: Vec<CString> = env::args()
        .map(|x| CString::new(x).unwrap())
        .collect();
    let mut c_argv: Vec<*const u8>;
    c_argv = c_args.iter().map(|x| x.as_bytes().as_ptr()).collect();
    c_argv.push(null());
    debug!("Executing {:?} {:?}", lithos_tree,
        env::args().collect::<Vec<_>>());
    unsafe {
        execv(c_exe.as_ptr(), c_argv.as_ptr() as *mut *const i8);
    }
    panic!("Can't reexec myself: {}", IoError::last_os_error());
}

struct Binaries {
    lithos_tree: Rc<PathBuf>,
    lithos_knot: Rc<PathBuf>,
}

fn get_binaries() -> Option<Binaries> {
    let dir = match env::current_exe().ok()
        .and_then(|x| x.parent().map(|y| y.to_path_buf()))
    {
        Some(dir) => dir,
        None => return None,
    };
    let bin = Binaries {
        lithos_tree: Rc::new(dir.join("lithos_tree")),
        lithos_knot: Rc::new(dir.join("lithos_knot")),
    };
    if !metadata(&*bin.lithos_tree).map(|x| x.is_file()).unwrap_or(false) {
        write!(&mut stderr(), "Can't find lithos_tree binary").unwrap();
        return None;
    }
    if !metadata(&*bin.lithos_knot).map(|x| x.is_file()).unwrap_or(false) {
        write!(&mut stderr(), "Can't find lithos_knot binary").unwrap();
        return None;
    }
    return Some(bin);
}

fn main() {

    signal::block_all();

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
            (write!(&mut stderr(), "Fatal error: {}\n", e)).ok();
            exit(1);
        }
    }
}
