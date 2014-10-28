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


use std::os::args;
use std::rc::Rc;
use std::io::stderr;
use std::io::IoError;
use std::io::fs::File;
use std::os::getenv;
use std::from_str::FromStr;
use std::io::fs::{readdir, mkdir, mkdir_recursive, rmdir, rmdir_recursive};
use std::os::{set_exit_status, self_exe_path, self_exe_name};
use std::io::FilePermission;
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
use serialize::json::Json;

use argparse::{ArgumentParser, Store};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::child_config::ChildConfig;
use lithos::monitor::{Monitor, Executor, Killed, Reboot};
use lithos::monitor::{PrepareResult, Run, Error};
use lithos::container::Command;
use lithos::mount::{bind_mount, mount_private, unmount};
use lithos::mount::check_mount_point;
use lithos::signal;


struct Child {
    name: Rc<String>,
    global_file: Rc<Path>,
    child_config_serialized: Rc<String>,
    global_config: Rc<TreeConfig>,
}

impl Child {
    fn _prepare(&self) -> Result<(), String> {
        try_str!(mkdir(
            &self.global_config.state_dir.join(self.name.as_slice()),
            FilePermission::all()));
        return Ok(());
    }
}

impl Executor for Child {
    fn command(&self) -> Command
    {
        let mut cmd = Command::new((*self.name).clone(),
            self_exe_path().unwrap().join("lithos_knot"));
        cmd.keep_sigmask();

        // Name is first here, so it's easily visible in ps
        cmd.arg("--name");
        cmd.arg(self.name.as_slice());

        cmd.arg("--global-config");
        cmd.arg(&*self.global_file);
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
        cmd.container(false);
        return cmd;
    }
    fn prepare(&self) -> PrepareResult {
        match self._prepare() {
            Ok(()) => Run,
            Err(x) => Error(x),
        }
    }
    fn finish(&self) -> bool {
        let st_dir = self.global_config.state_dir.join(self.name.as_slice());
        rmdir_recursive(&st_dir)
            .map_err(|e| error!("Error removing dir {}: {}",
                                 st_dir.display(), e))
            .ok();
        return true;
    }
}
struct UnidentifiedChild {
    name: Rc<String>,
    global_config: Rc<TreeConfig>,
}

impl Executor for UnidentifiedChild {
    fn command(&self) -> Command {
        unreachable!();
    }
    fn finish(&self) -> bool {
        let st_dir = self.global_config.state_dir.join(self.name.as_slice());
        rmdir_recursive(&st_dir)
            .map_err(|e| error!("Error removing dir {}: {}",
                                 st_dir.display(), e))
            .ok();
        return false;
    }
}

fn check_config(cfg: &TreeConfig) -> Result<(), String> {
    if !Path::new(cfg.devfs_dir.as_slice()).exists() {
        return Err(format!(
            "Devfs dir ({}) must exist and contain device nodes",
            cfg.devfs_dir));
    }
    return Ok(());
}

fn global_init(cfg: &TreeConfig) -> Result<(), String> {
    try_str!(mkdir_recursive(&cfg.state_dir,
        FilePermission::from_bits_truncate(0o755)));

    try_str!(mkdir_recursive(&cfg.mount_dir,
        FilePermission::from_bits_truncate(0o755)));

    let mut bind = true;
    let mut private = true;
    try!(check_mount_point(
        cfg.mount_dir.display().as_maybe_owned().as_slice(), |m| {
        bind = false;
        if m.is_private() {
            private = false;
        }
    }));

    if bind {
        try_str!(bind_mount(&cfg.mount_dir, &cfg.mount_dir));
    }
    if private {
        try_str!(mount_private(&cfg.mount_dir));
    }

    return Ok(());
}

fn global_cleanup(cfg: &TreeConfig) {
    unmount(&cfg.mount_dir).unwrap_or_else(
        |e| error!("Error unmouting mount dir {}: {}",
                   cfg.mount_dir.display(), e));
    rmdir(&cfg.mount_dir).unwrap_or_else(
        |e| error!("Error removing mount dir {}: {}",
                   cfg.mount_dir.display(), e));

    rmdir_recursive(&cfg.state_dir).unwrap_or_else(
        |e| error!("Error removing state dir {}: {}",
                   cfg.state_dir.display(), e));
}

fn discard<E>(_: E) { }

fn _read_args(procfsdir: &Path, global_config: &Path)
    -> Result<(String, Json), ()>
{
    let mut f = try!(File::open(&procfsdir.join("cmdline")).map_err(discard));
    let line = try!(f.read_to_string().map_err(discard));
    let args: Vec<&str> = line.as_slice().splitn(6, '\0').collect();
    if args.len() != 7
       || Path::new(args[0]).filename_str() != Some("lithos_knot")
       || args[1] != "--name"
       || args[3] != "--global-config"
       || args[4].as_bytes() != global_config.container_as_bytes()
       || args[5] != "--config"
    {
       return Err(());
    }
    return Ok((
        args[2].to_string(),
        try!(json::from_str(args[6]).map_err(discard)),
        ));
}

fn _is_child(procfsdir: &Path, mypid: i32) -> bool
{
    let ppid_regex = regex!(r"^\d+\s+\([^)]*\)\s+\S+\s+(\d+)\s");
    let stat =
        File::open(&procfsdir.join("stat")).ok()
        .and_then(|mut f| f.read_to_string().ok());
    if stat.is_none() {
        return false;
    }

    let ppid = ppid_regex.captures(stat.unwrap().as_slice())
               .and_then(|c| FromStr::from_str(c.at(1)));
    if ppid != Some(mypid) {
        return false;
    }
    return true;
}


fn _get_name(procfsdir: &Path, global_config: &Path)
    -> Option<(String, String, Json)>
{
    let (name, cfg) =  match _read_args(procfsdir, global_config) {
        Ok(tuple) => tuple,
        Err(_) => { return None; }
    };

    let name_regex = regex!(r"^([\w-]+)\.\d+$");
    match name_regex.captures(name.as_slice()) {
        Some(captures)
        => Some((captures.at(0).to_string(), captures.at(1).to_string(), cfg)),
        None => None,
    }
}

fn run(config_file: Path) -> Result<(), String> {
    let cfg: Rc<TreeConfig> = Rc::new(try_str!(parse_config(&config_file,
        &*TreeConfig::validator(), Default::default())));

    try!(check_config(&*cfg));

    let mut children: HashMap<Path, (ChildConfig, Json, Rc<String>)>;
    children = HashMap::new();
    debug!("Checking child dir {}", cfg.config_dir);
    let configdir = Path::new(cfg.config_dir.as_slice());
    let dirlist = try_str!(readdir(&configdir));
    for child_fn in dirlist.into_iter() {
        match (child_fn.filestem_str(), child_fn.extension_str()) {
            (Some(""), _) => continue,  // Hidden files
            (_, Some("yaml")) => {}
            _ => continue,  // Non-yaml, old, whatever, files
        }
        debug!("Adding {}", child_fn.display());
        let child_cfg: ChildConfig = try_str!(parse_config(&child_fn,
            &*ChildConfig::validator(), Default::default()));
        let child_cfg_string = Rc::new(json::encode(&child_cfg));
        let child_json = json::from_str(child_cfg_string.as_slice()).unwrap();
        children.insert(child_fn, (child_cfg, child_json, child_cfg_string));
    }

    try!(global_init(&*cfg));

    let mut mon = Monitor::new("lithos-tree".to_string());
    let config_file = Rc::new(config_file);
    let mypid = unsafe { getpid() };

    // Recover old workers
    for ppath in readdir(&Path::new("/proc"))
        .ok().expect("Can't read procfs").iter()
    {
        let pid: pid_t;
        pid = match ppath.filename_str().and_then(FromStr::from_str) {
            Some(pid) => pid,
            None => continue,
        };
        if !_is_child(ppath, mypid) {
            continue;
        }
        let (fullname, childname, current_config) = match _get_name(
                ppath, &*config_file)
        {
            Some(tup) => tup,
            None => {
                warn!("Undefined child, pid: {}. Sending SIGTERM...",
                      pid);
                //signal::send_signal(pid, signal::SIGTERM as int);
                continue;
            }
        };
        let fullname = Rc::new(fullname);
        let cfg_path = configdir.join(childname + ".yaml");
        match children.find(&cfg_path) {
            Some(&(ref child_cfg, ref json, ref config)) => {
                mon.add(fullname.clone(), box Child {
                    name: fullname.clone(),
                    global_file: config_file.clone(),
                    global_config: cfg.clone(),
                    child_config_serialized: config.clone(),
                    }, Duration::seconds(1),
                    Some((pid, get_time())));
                if *json != current_config {
                    warn!("Config mismatch: {}, pid: {}. Upgrading...",
                          fullname, pid);
                    signal::send_signal(pid, signal::SIGTERM as int);
                }
            }
            None => {
                warn!("Undefined child name: {}, pid: {}. Sending SIGTERM...",
                      fullname, pid);
                mon.add(fullname.clone(), box UnidentifiedChild {
                    name: fullname,
                    global_config: cfg.clone(),
                    }, Duration::seconds(0),
                    Some((pid, get_time())));
                signal::send_signal(pid, signal::SIGTERM as int);
            }
        };
    }

    // Remove dangling state dirs
    for ppath in readdir(&cfg.state_dir)
        .ok().expect("Can't read state dir").iter()
    {
        if let Some(name) = ppath.filename_str() {
            if mon.has(&Rc::new(name.to_string())) {
                continue;
            }
            warn!("Dangling state dir {}. Deleting...", ppath.display());
            rmdir_recursive(ppath)
                .map_err(|e| error!("Can't remove dangling dir {}: {}",
                    ppath.display(), e))
                .ok();
        }
    }

    // Schedule new workers
    for (path, (child_cfg, _json, child_cfg_string)) in children.into_iter() {
        let path = Rc::new(path);
        let stem = path.filestem_str().unwrap();
        for i in range(0, child_cfg.instances) {
            let name = Rc::new(format!("{}.{}", stem, i));
            if mon.has(&name) {
                continue;
            }
            mon.add(name.clone(), box Child {
                name: name,
                global_file: config_file.clone(),
                global_config: cfg.clone(),
                child_config_serialized: child_cfg_string.clone(),
            }, Duration::seconds(1),
            None);
        }
    }
    mon.allow_reboot();
    match mon.run() {
        Killed => {}
        Reboot => {
            reexec_myself();
        }
    }

    global_cleanup(&*cfg);

    return Ok(());
}

fn reexec_myself() -> ! {
    let args = args();
    let exe = self_exe_name().unwrap();
    let c_exe = exe.to_c_str();
    let c_args: Vec<CString> = args.iter().map(|x| x.to_c_str()).collect();
    let mut c_argv: Vec<*const u8>;
    c_argv = c_args.iter().map(|x| x.as_bytes().as_ptr()).collect();
    c_argv.push(null());
    debug!("Executing {} {}", exe.display(), args);
    unsafe {
        execv(c_exe.as_ptr(), c_argv.as_ptr() as *mut *const i8);
    }
    fail!("Can't reexec myself: {}", IoError::last_error());
}

fn check_binaries() -> bool {
    let dir = match self_exe_path() {
        Some(dir) => dir,
        None => return false,
    };
    if !dir.join("lithos_tree").exists() {
        error!("Can't find lithos_tree binary");
        return false;
    }
    if !dir.join("lithos_knot").exists() {
        error!("Can't find lithos_knot binary");
        return false;
    }
    return true;
}

fn main() {

    signal::block_all();

    if !check_binaries() {
        set_exit_status(127);
        return;
    }
    let mut config_file = Path::new("/etc/lithos.yaml");
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Runs tree of processes");
        ap.refer(&mut config_file)
          .add_option(["-C", "--config"], box Store::<Path>,
            "Name of the global configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match run(config_file) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
