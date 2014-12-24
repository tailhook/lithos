#![feature(phase, macro_rules, if_let)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;
extern crate regex;
#[phase(plugin)] extern crate regex_macros;
extern crate time;
extern crate debug;

extern crate argparse;
extern crate quire;
#[phase(plugin, link)] extern crate lithos;


use std::rc::Rc;
use std::io::IoError;
use std::io::fs::File;
use std::os::{getenv, args};
use std::io::stdio::{stdout, stderr};
use std::from_str::FromStr;
use std::io::fs::{readdir, rmdir};
use std::os::{set_exit_status, self_exe_path};
use std::ptr::null;
use std::time::Duration;
use std::path::BytesContainer;
use std::io::fs::PathExtensions;
use std::c_str::{ToCStr, CString};
use std::default::Default;
use std::collections::HashMap;
use time::get_time;
use libc::pid_t;
use libc::funcs::posix88::unistd::{getpid, execv};
use serialize::json;

use argparse::{ArgumentParser, Store};
use quire::parse_config;

use lithos::setup::clean_child;
use lithos::master_config::{MasterConfig, create_master_dirs};
use lithos::tree_config::TreeConfig;
use lithos::child_config::ChildConfig;
use lithos::container_config::Daemon;
use lithos::monitor::{Monitor, Executor, Killed, Reboot};
use lithos::container::Command;
use lithos::utils::{clean_dir};
use lithos::signal;
use lithos::cgroup;


struct Child {
    name: Rc<String>,
    master_file: Rc<Path>,
    child_config_serialized: Rc<String>,
    master_config: Rc<MasterConfig>,
    child_config: Rc<ChildConfig>,
    root_binary: Rc<Path>,
}

impl Executor for Child {
    fn command(&self) -> Command
    {
        let mut cmd = Command::new((*self.name).clone(), &*self.root_binary);
        cmd.keep_sigmask();

        // Name is first here, so it's easily visible in ps
        cmd.arg("--name");
        cmd.arg(self.name.as_slice());

        cmd.arg("--master");
        cmd.arg(&*self.master_file);
        cmd.arg("--config");
        cmd.arg(self.child_config_serialized.as_slice());
        cmd.set_env("TERM".to_string(),
                    getenv("TERM").unwrap_or("dumb".to_string()));
        if let Some(x) = getenv("RUST_LOG") {
            cmd.set_env("RUST_LOG".to_string(), x);
        }
        if let Some(x) = getenv("RUST_BACKTRACE") {
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
    if !cfg.devfs_dir.exists() {
        return Err(format!(
            "Devfs dir ({}) must exist and contain device nodes",
            cfg.devfs_dir.display()));
    }
    return Ok(());
}

fn global_init(master: &MasterConfig) -> Result<(), String> {
    try!(create_master_dirs(&*master));
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
    let line = try!(File::open(&Path::new(format!("/proc/{}/cmdline", pid)))
                    .and_then(|mut f| f.read_to_string())
                    .map_err(discard));
    let args: Vec<&str> = line.as_slice().splitn(7, '\0').collect();
    if args.len() != 8
       || Path::new(args[0]).filename_str() != Some("lithos_knot")
       || args[1] != "--name"
       || args[3] != "--master"
       || args[4].as_bytes() != global_config.container_as_bytes()
       || args[5] != "--config"
       || args[7] != ""
    {
       return Err(());
    }
    return Ok((args[2].to_string(), args[6].to_string()));
}

fn _is_child(pid: pid_t, ppid: pid_t) -> bool {
    let ppid_regex = regex!(r"^\d+\s+\([^)]*\)\s+\S+\s+(\d+)\s");
    let stat = File::open(&Path::new(format!("/proc/{}/stat", pid)))
               .and_then(|mut f| f.read_to_string());
    if stat.is_err() {
        return false;
    }
    let stat = stat.unwrap();
    return Some(ppid) == ppid_regex.captures(stat.as_slice())
                         .and_then(|c| FromStr::from_str(c.at(1)));
}


fn check_process(cfg: &MasterConfig) -> Result<(), String> {
    let mypid = unsafe { getpid() };
    let pid_file = cfg.runtime_dir.join("master.pid");
    if pid_file.exists() {
        match File::open(&pid_file)
            .and_then(|mut f| f.read_to_string())
            .map(|s| FromStr::from_str(s.as_slice()))
        {
            Ok(Some::<pid_t>(pid)) if pid == mypid => {
                return Ok(());
            }
            Ok(Some(pid)) => {
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
        .and_then(|mut f| f.write_uint(unsafe { getpid() } as uint))
        .map_err(|e| format!("Can't write file {}: {}",
                             pid_file.display(), e)));
    return Ok(());
}

fn recover_processes(master: &Rc<MasterConfig>, mon: &mut Monitor,
    configs: &mut HashMap<Rc<String>, Child>, config_file: &Rc<Path>)
{
    let mypid = unsafe { getpid() };

    // Recover old workers
    for pid in readdir(&Path::new("/proc"))
        .map_err(|e| format!("Can't read procfs: {}", e))
        .unwrap_or(Vec::new())
        .into_iter()
        .filter_map(|p| p.filename_str().and_then(FromStr::from_str))
    {
        if !_is_child(pid, mypid) {
            continue;
        }
        if let Ok((name, cfg_text)) = _read_args(pid, &**config_file) {
            let cfg = json::decode(cfg_text.as_slice())
                .map_err(|e| warn!(
                    "Error parsing recover config, pid {}, name {}: {}",
                    pid, name, e))
                .ok();
            let name = Rc::new(name);
            match configs.pop(&name) {
                Some(child) => {
                    if Some(&*child.child_config) != cfg.as_ref() {
                        warn!("Config mismatch: {}, pid: {}. Upgrading...",
                              name, pid);
                        signal::send_signal(pid, signal::SIGTERM as int);
                    }
                    mon.add(name.clone(), box child, Duration::seconds(1),
                        Some((pid, get_time())));
                }
                None => {
                    warn!("Undefined child name: {}, pid: {}. Sending SIGTERM...",
                          name, pid);
                    mon.add(name.clone(), box UnidentifiedChild {
                        name: name,
                        master_config: master.clone(),
                        }, Duration::seconds(0),
                        Some((pid, get_time())));
                    signal::send_signal(pid, signal::SIGTERM as int);
                }
            };
        } else {
            warn!("Undefined child, pid: {}. Sending SIGTERM...",
                  pid);
            signal::send_signal(pid, signal::SIGTERM as int);
            continue;
        }
    }
}

fn remove_dangling_state_dirs(mon: &Monitor, master: &MasterConfig) {
    for tree in readdir(&master.runtime_dir.join(&master.state_dir))
        .map_err(|e| error!("Can't read state dir: {}", e))
        .unwrap_or(Vec::new())
        .into_iter()
    {
        debug!("Checking tree dir: {}", tree.display());
        let mut valid_dirs = 0u;
        if let Some(tree_name) = tree.filename_str() {
            for cont in readdir(&tree)
                .map_err(|e| format!("Can't read state dir: {}", e))
                .unwrap_or(Vec::new())
                .into_iter()
            {
                if let Some(proc_name) = cont.filename_str() {
                    let name = Rc::new(format!("{}/{}", tree_name, proc_name));
                    debug!("Checking process dir: {}", name);
                    if mon.has(&name) {
                        valid_dirs += 1;
                        continue;
                    } else if proc_name.starts_with("cmd.") {
                        debug!("Checking command dir: {}", name);
                        let pid = regex!(r"\.\(\d+\)$")
                            .captures(proc_name)
                            .and_then(|c| FromStr::from_str(c.at(1)));
                        if let Some(pid) = pid {
                            if signal::is_process_alive(pid) {
                                valid_dirs += 1;
                                continue;
                            }
                        }
                    }
                }
                warn!("Dangling state dir {}. Deleting...", cont.display());
                clean_dir(&cont, true)
                    .map_err(|e| error!(
                        "Can't remove dangling state dir {}: {}",
                        cont.display(), e))
                    .ok();
            }
        }
        debug!("Tree dir {} has {} valid subdirs", tree.display(), valid_dirs);
        if valid_dirs > 0 {
            continue;
        }
        warn!("Empty tree dir {}. Deleting...", tree.display());
        clean_dir(&tree, true)
            .map_err(|e| error!("Can't empty state dir {}: {}",
                tree.display(), e))
            .ok();
    }
}

fn _rm_cgroup(dir: &Path) {
    if let Err(e) = rmdir(dir) {
        let procs = File::open(&dir.join("cgroup.procs"))
            .and_then(|mut f| f.read_to_string());
        error!("Error removing cgroup: {} (processes {})",
            e, procs);
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
    let child_group_regex = regex!(r"^([\w-]+):([\w-]+\.\d+)\.scope$");
    let cmd_group_regex = regex!(r"^([\w-]+):cmd\.[\w-]+\.(\d+)\.scope$");
    let cgroup_filename = master.cgroup_name.as_ref().map(|x| x.as_slice());

    // Loop over all controllers in case someone have changed config
    for cgrp in cgroups.all_groups.iter() {
        let cgroup::CGroupPath(ref folder, ref path) = **cgrp;
        let ctr_dir = cgroup_base.join(folder.as_slice()).join(
            &path.path_relative_from(&root_path).unwrap());
        if path.filename_str() == cgroup_filename {
            debug!("Checking controller dir: {}", ctr_dir.display());
        } else {
            debug!("Skipping controller dir: {}", ctr_dir.display());
            continue;
        }
        for child_dir in readdir(&ctr_dir)
            .map_err(|e| debug!("Can't read controller {} dir: {}",
                                ctr_dir.display(), e))
            .unwrap_or(Vec::new())
            .into_iter()
        {
            if !child_dir.is_dir() {
                continue;
            }
            let filename = child_dir.filename_str();
            if filename.is_none() {
                warn!("Wrong filename in cgroup: {}", child_dir.display());
                continue;
            }
            let filename = filename.unwrap();
            if let Some(capt) = child_group_regex.captures(filename) {
                let name = format!("{}/{}", capt.at(1), capt.at(2));
                if !mon.has(&Rc::new(name)) {
                    _rm_cgroup(&child_dir);
                }
            } else if let Some(capt) = cmd_group_regex.captures(filename) {
                let pid = FromStr::from_str(capt.at(2));
                if pid.is_none() || !signal::is_process_alive(pid.unwrap()) {
                    _rm_cgroup(&child_dir);
                }
            } else {
                warn!("Skipping wrong group {}", child_dir.display());
                continue;
            }
        }
    }
}

fn run(config_file: Path, bin: &Binaries) -> Result<(), String> {
    let master: Rc<MasterConfig> = Rc::new(try_str!(parse_config(&config_file,
        &*MasterConfig::validator(), Default::default())));
    try!(check_master_config(&*master));
    try!(global_init(&*master));

    let config_file = Rc::new(config_file);
    let mut mon = Monitor::new("lithos-tree".to_string());

    info!("Reading tree configs from {}", master.config_dir.display());
    let mut configs = read_configs(&master, bin, &config_file);

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
            reexec_myself(&*bin.lithos_tree);
        }
    }

    global_cleanup(&*master);

    return Ok(());
}

fn read_configs(master: &Rc<MasterConfig>, bin: &Binaries,
    master_file: &Rc<Path>)
    -> HashMap<Rc<String>, Child>
{
    let tree_validator = TreeConfig::validator();
    let name_re = regex!(r"^([\w-]+)\.yaml$");
    readdir(&master.config_dir)
        .map_err(|e| { error!("Can't read config dir: {}", e); e })
        .unwrap_or(Vec::new())
        .into_iter()
        .filter_map(|f| {
            let name = match f.filename_str().and_then(|s| name_re.captures(s))
            {
                Some(capt) => capt.at(1),
                None => return None,
            };
            debug!("Reading config: {}", f.display());
            parse_config(&f, &*tree_validator, Default::default())
                .map_err(|e| warn!("Can't read config {}: {}", f.display(), e))
                .map(|cfg: TreeConfig| (name.to_string(), cfg))
                .ok()
        })
        .flat_map(|(name, tree)| {
            read_subtree(master, bin, master_file, &name, Rc::new(tree))
            .into_iter()
        })
        .collect()
}

fn read_subtree<'x>(master: &Rc<MasterConfig>,
    bin: &Binaries, master_file: &Rc<Path>,
    tree_name: &String, tree: Rc<TreeConfig>)
    -> Vec<(Rc<String>, Child)>
{
    let name_re = regex!(r"^([\w-]+)\.yaml$");
    let child_validator = ChildConfig::validator();
    debug!("Reading child dir {}", tree.config_dir.display());
    readdir(&tree.config_dir)
        .map_err(|e| { error!("Can't read config dir: {}", e); e })
        .unwrap_or(Vec::new())
        .into_iter()
        .filter_map(|f| {
            let name = match f.filename_str().and_then(|s| name_re.captures(s))
            {
                Some(capt) => capt.at(1),
                None => return None,
            };
            parse_config(&f, &*child_validator, Default::default())
                .map_err(|e| warn!("Can't read config {}: {}", f.display(), e))
                .map(|cfg: ChildConfig| (name.to_string(), cfg))
                .ok()
        })
        .filter(|&(_, ref child)| child.kind == Daemon)
        .flat_map(|(child_name, child)| {
            let child_string = Rc::new(json::encode(&child));
            let child = Rc::new(child);
            let items: Vec<(Rc<String>, Child)> = range(0, child.instances)
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
        mon.add(name.clone(), box child, Duration::seconds(2), None);
    }
}

fn reexec_myself(lithos_tree: &Path) -> ! {
    let args = args();
    let c_exe = lithos_tree.to_c_str();
    let c_args: Vec<CString> = args.iter().map(|x| x.to_c_str()).collect();
    let mut c_argv: Vec<*const u8>;
    c_argv = c_args.iter().map(|x| x.as_bytes().as_ptr()).collect();
    c_argv.push(null());
    debug!("Executing {} {}", lithos_tree.display(), args);
    unsafe {
        execv(c_exe.as_ptr(), c_argv.as_ptr() as *mut *const i8);
    }
    fail!("Can't reexec myself: {}", IoError::last_error());
}

struct Binaries {
    lithos_tree: Rc<Path>,
    lithos_knot: Rc<Path>,
}

fn get_binaries() -> Option<Binaries> {
    let dir = match self_exe_path() {
        Some(dir) => dir,
        None => return None,
    };
    let bin = Binaries {
        lithos_tree: Rc::new(dir.join("lithos_tree")),
        lithos_knot: Rc::new(dir.join("lithos_knot")),
    };
    if !bin.lithos_tree.is_file() {
        error!("Can't find lithos_tree binary");
        return None;
    }
    if !bin.lithos_knot.is_file() {
        error!("Can't find lithos_knot binary");
        return None;
    }
    return Some(bin);
}

pub struct Options {
    pub config_file: Path,
}

impl Options {
    pub fn parse_args() -> Result<Options, int> {
        Options::parse_specific_args(args(), &mut stdout(), &mut stderr())
    }
    pub fn parse_specific_args(args: Vec<String>,
        stdout: &mut Writer, stderr: &mut Writer)
        -> Result<Options, int>
    {
        let mut options = Options {
            config_file: Path::new("/etc/lithos.yaml"),
        };
        let mut ap = ArgumentParser::new();
        ap.set_description("Runs tree of processes");
        ap.refer(&mut options.config_file)
          .add_option(["-C", "--config"], box Store::<Path>,
            "Name of the global configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        match ap.parse(args, stdout, stderr) {
            Ok(()) => Ok(options),
            Err(x) => Err(x),
        }
    }
}

fn main() {

    signal::block_all();

    let bin = match get_binaries() {
        Some(bin) => bin,
        None => {
            set_exit_status(127);
            return;
        }
    };
    let options = match Options::parse_args() {
        Ok(options) => options,
        Err(x) => {
            set_exit_status(x);
            return;
        }
    };
    match run(options.config_file, &bin) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
