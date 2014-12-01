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


use std::io::stderr;
use std::io::IoError;
use std::io::ALL_PERMISSIONS;
use std::os::{set_exit_status, self_exe_path};
use std::from_str::FromStr;
use std::io::fs::{copy, rmdir_recursive, mkdir, readdir, rename};
use std::io::fs::{readlink, symlink, File};
use std::io::fs::PathExtensions;
use std::default::Default;
use std::io::process::{Command, InheritFd, ExitStatus};

use argparse::{ArgumentParser, Store, StoreOption, StoreTrue};
use quire::parse_config;

use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;
use lithos::child_config::ChildConfig;
use lithos::signal;

fn copy_dir(source: &Path, target: &Path) -> Result<(), IoError> {
    let name_re = regex!(r"^([\w-]+)\.yaml$");
    let mut tmpdir = Path::new(target.dirname())
        .join(b".tmp.".to_vec() + target.filename().unwrap());
    if tmpdir.exists() {
        try!(rmdir_recursive(&tmpdir));
    }
    try!(mkdir(&tmpdir, ALL_PERMISSIONS));
    for file in try!(readdir(source))
        .into_iter()
        .filter(|f| f.filename_str()
                     .map(|s| name_re.is_match(s))
                     .unwrap_or(false))
    {
        try!(copy(&file, &tmpdir.join(file.filename().unwrap())));
    }
    try!(rename(&tmpdir, target));
    return Ok(());
}


fn switch_config(master_cfg: Path, tree_name: String, config_dir: Path)
    -> Result<(), String>
{
    match Command::new(self_exe_path().unwrap().join("lithos_check"))
        .stdin(InheritFd(0))
        .stdout(InheritFd(1))
        .stderr(InheritFd(2))
        .arg("--config")
        .arg(&master_cfg)
        .arg("--tree")
        .arg(tree_name.as_slice())
        .arg("--config-dir")
        .arg(&config_dir)
        .output()
    {
        Ok(ref po) if po.status == ExitStatus(0) => { }
        Ok(ref po) => {
            return Err(format!(
                "Configuration check failed with exit status: {}",
                po.status));
        }
        Err(e) => {
            return Err(format!("Can't check configuration: {}", e));
        }
    }
    info!("Checked. Proceeding");

    let master: MasterConfig = match parse_config(&master_cfg,
        &*MasterConfig::validator(), Default::default())
    {
        Ok(cfg) => cfg,
        Err(e) => {
            return Err(format!("Can't parse master config: {}", e));
        }
    };
    let tree_fn = master.config_dir.join(tree_name + ".yaml");
    let tree: TreeConfig = match parse_config(&tree_fn,
        &*TreeConfig::validator(), Default::default())
    {
        Ok(cfg) => cfg,
        Err(e) => {
            return Err(format!("Can't parse tree config: {}", e));
        }
    };

    let config_fn = config_dir.filename_str().unwrap();
    let target_fn = Path::new(tree.config_dir.dirname()).join(config_fn);
    if target_fn == tree.config_dir {
        return Err(format!(concat!(
            "Target config dir ({}) is the current dir ({}). ",
            "You must have unique name for each new configuration"),
            target_fn.display(), tree.config_dir.display()));
    }
    let lnk = readlink(&tree.config_dir);
    match lnk.as_ref().map(|f| f.dirname()) {
        Ok(b".") => {},
        _ => return Err(format!(concat!(
            "The path {} must be a directory which points to the directory ",
            "at the same level of hierarchy."), tree.config_dir.display())),
    };
    if lnk.unwrap().filename() == target_fn.filename() {
        warn!("Target config dir is the active one. Nothing to do.");
        return Ok(());
    }
    if target_fn.exists() {
        warn!(concat!("Target directory {} exists. ",
            "Probably already copied. Skipping copying step."),
            target_fn.display());
    } else {
        info!("Seems everything nice. Copying");
        match copy_dir(&config_dir, &target_fn) {
            Ok(()) => {}
            Err(e) => return Err(format!("Error copying config dir: {}", e)),
        }
    }
    info!("Ok files are there. Making symlink");
    let symlink_fn = tree.config_dir.with_filename(
        b".tmp.".to_vec() + tree.config_dir.filename().unwrap());
    try!(symlink(&Path::new(config_fn), &symlink_fn)
        .map_err(|e| format!("Error symlinking dir: {}", e)));
    try!(rename(&symlink_fn, &tree.config_dir)
        .map_err(|e| format!("Error replacing symlink: {}", e)));

    info!("Done. Sending SIGQUIT to lithos_tree");
    let pid_file = master.runtime_dir.join("master.pid");
    let pid = match File::open(&pid_file)
            .and_then(|mut f| f.read_to_string())
            .ok()
            .and_then(|s| FromStr::from_str(s.as_slice())) {
        Some(pid) if signal::is_process_alive(pid) => {
            signal::send_signal(pid, signal::SIGQUIT as int);
        }
        Some(pid) => {
            warn!("Process with pid {} is not running...", pid);
        }
        None => {
            warn!("Can't read pid file {}. Probably daemon is not running.",
                pid_file.display());
        }
    };

    return Ok(());
}


fn main() {
    let mut master_config = Path::new("/etc/lithos.yaml");
    let mut verbose = false;
    let mut config_dir = Path::new("");
    let mut tree_name = "".to_string();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Checks if lithos configuration is ok");
        ap.refer(&mut master_config)
          .add_option(["--master"], box Store::<Path>,
            "Name of the master configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut verbose)
          .add_option(["-v", "--verbose"], box StoreTrue,
            "Verbose configuration");
        ap.refer(&mut tree_name)
          .add_argument("tree", box Store::<String>,
            "Name of the tree which configuration will be switched for")
          .required()
          .metavar("NAME");
        ap.refer(&mut config_dir)
          .add_argument("dir", box Store::<Path>, "
            Name of the configuration directory to switch to. It doesn't
            have to be a directory inside `config-dir`, and it will be copied
            there. However, if directory with the same name exists in the
            `config-dir` it's assumed that it's already copied and will not
            be updated. Be sure to use unique directory for each deployment.
            ")
          .required()
          .metavar("PATH");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match switch_config(master_config, tree_name, config_dir) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
