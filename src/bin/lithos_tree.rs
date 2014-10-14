#![feature(phase, macro_rules)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;
extern crate regex;
#[phase(plugin)] extern crate regex_macros;

extern crate argparse;
extern crate quire;


use std::rc::Rc;
use std::io::stderr;
use std::io::{IoResult, IoError};
use std::io::fs::File;
use std::os::getenv;
use std::io::BufferedReader;
use std::from_str::FromStr;
use std::str::from_utf8;
use std::io::fs::{readdir, mkdir_recursive, rmdir, rmdir_recursive};
use std::os::{set_exit_status, self_exe_path};
use std::io::FilePermission;
use std::default::Default;
use std::collections::HashMap;
use libc::consts::os::posix88::EINVAL;
use libc::funcs::posix88::unistd::getpid;
use libc::pid_t;

use argparse::{ArgumentParser, Store};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;
use lithos::monitor::{Monitor, Executor};
use lithos::container::Command;
use lithos::mount::{bind_mount, mount_private, unmount};
use lithos::signal;

#[path="../mod.rs"]
mod lithos;


struct Child {
    name: String,
    global_config: Rc<Path>,
    container_file: Rc<Path>,
    container_config: Rc<ContainerConfig>,
}

impl Executor for Child {
    fn command(&self) -> Command
    {
        let mut cmd = Command::new(
            self_exe_path().unwrap().join("lithos_knot"));
        cmd.keep_sigmask();

        // Name is first here, so it's easily visible in ps
        cmd.arg("--name");
        cmd.arg(self.name.as_slice());

        cmd.arg("--global-config");
        cmd.arg(&*self.global_config);
        cmd.arg("--container-config");
        cmd.arg(&*self.container_file);
        cmd.set_env("TERM".to_string(),
                    getenv("TERM").unwrap_or("dumb".to_string()));
        getenv("RUST_LOG").map(|x| cmd.set_env("RUST_LOG".to_string(), x));
        cmd.container(false);
        return cmd;
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
    try_str!(mkdir_recursive(&Path::new(cfg.state_dir.as_slice()),
        FilePermission::from_bits_truncate(0o755)));

    let mntdir = Path::new(cfg.mount_dir.as_slice());
    try_str!(mkdir_recursive(&mntdir,
        FilePermission::from_bits_truncate(0o755)));
    try_str!(bind_mount(&mntdir, &mntdir));
    try_str!(mount_private(&mntdir));
    return Ok(());
}

fn global_cleanup(cfg: &TreeConfig) {
    let mntdir = Path::new(cfg.mount_dir.as_slice());
    unmount(&mntdir).unwrap_or_else(
        |e| error!("Error unmouting mount dir {}: {}", cfg.mount_dir, e));
    rmdir(&mntdir).unwrap_or_else(
        |e| error!("Error removing mount dir {}: {}", cfg.mount_dir, e));

    rmdir_recursive(&Path::new(cfg.state_dir.as_slice())).unwrap_or_else(
        |e| error!("Error removing state dir {}: {}", cfg.state_dir, e));
}

fn any_error<E>(_: E) { }

fn _read_args(procfsdir: &Path) -> Result<(String, String, String), ()> {
    let f = try!(File::open(&procfsdir.join("cmdline")).map_err(any_error));
    let mut buf = BufferedReader::new(f);
    let exe = try!(String::from_utf8(
        try!(buf.read_until(0).map_err(any_error))).map_err(any_error));
    let name_flag = try!(String::from_utf8(
        try!(buf.read_until(0).map_err(any_error))).map_err(any_error));
    let name = try!(String::from_utf8(
        try!(buf.read_until(0).map_err(any_error))).map_err(any_error));
    return Ok((exe, name_flag, name));
}

fn _get_name(procfsdir: &Path, mypid: i32) -> Option<(String, String)> {
    let ppid_regex = regex!(r"^\d+\s+\([^)]*\)\s+\S+\s+(\d+)\s");
    let stat =
        File::open(&procfsdir.join("stat")).ok()
        .and_then(|mut f| f.read_to_str().ok());
    if stat.is_none() {
        return None;
    }

    let ppid = ppid_regex.captures(stat.unwrap().as_slice())
               .and_then(|c| FromStr::from_str(c.at(1)));
    if ppid != Some(mypid) {
        return None;
    }

    let name = match _read_args(procfsdir) {
        Ok((exe, name_flag, name)) => {
            if Path::new(exe.as_slice()).filename_str() == Some("lithos_knot")
                && name_flag.as_slice() == "--name" {
                name
            } else {
                return None;
            }
        }
        _ => return None,
    };

    let name_regex = regex!(r"(\w+)\.\d+");
    return name_regex.captures(name.as_slice())
           .map(|captures| {
        (captures.at(0).to_string(), captures.at(1).to_string())
    });
}

fn run(config_file: Path) -> Result<(), String> {
    let cfg: TreeConfig = try_str!(parse_config(&config_file,
        TreeConfig::validator(), Default::default()));

    try!(check_config(&cfg));

    let mut children: HashMap<Path, Rc<ContainerConfig>> = HashMap::new();
    debug!("Checking child dir {}", cfg.config_dir);//.display());
    let configdir = Path::new(cfg.config_dir.as_slice());
    let dirlist = try_str!(readdir(&configdir));
    for child_fn in dirlist.move_iter() {
        match (child_fn.filestem_str(), child_fn.extension_str()) {
            (Some(""), _) => continue,  // Hidden files
            (_, Some("yaml")) => {}
            _ => continue,  // Non-yaml, old, whatever, files
        }
        debug!("Adding {}", child_fn.display());
        let child_cfg = try_str!(parse_config(&child_fn,
            ContainerConfig::validator(), Default::default()));
        children.insert(child_fn, Rc::new(child_cfg));
    }

    try!(global_init(&cfg));

    let mut mon = Monitor::new("lithos-tree".to_string());
    let config_file = Rc::new(config_file);
    let mypid = unsafe { getpid() };

    for ppath in readdir(&Path::new("/proc"))
        .ok().expect("Can't read procfs").iter() {
        let pid: pid_t;
        pid = match ppath.filename_str().and_then(FromStr::from_str) {
            Some(pid) => pid,
            None => continue,
        };
        let (fullname, childname) = match _get_name(ppath, mypid) {
            Some((fullname, childname)) => (fullname, childname),
            None => continue,
        };
        let cfg_path = configdir.join(childname);
        let cfg = match children.find(&cfg_path) {
            Some(cfg) => cfg,
            None => {
                warn!("Undefined child name: {}, pid: {}. Sending SIGTERM...",
                      fullname, pid);
                signal::send_signal(pid, signal::SIGTERM as int);
                continue;
            }
        };
        mon.add(fullname.clone(), box Child {
            name: fullname,
            global_config: config_file.clone(),
            container_file: Rc::new(cfg_path),
            container_config: cfg.clone(),
        });
    }

    for (path, cfg) in children.move_iter() {
        let path = Rc::new(path);
        let stem = path.filestem_str().unwrap();
        for i in range(0, cfg.instances) {
            let name = format!("{}.{}", stem, i);
            if mon.has(&name) {
                continue;
            }
            mon.add(name.clone(), box Child {
                name: name,
                global_config: config_file.clone(),
                container_file: path.clone(),
                container_config: cfg.clone(),
            });
        }
    }
    mon.run();

    global_cleanup(&cfg);

    return Ok(());
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
