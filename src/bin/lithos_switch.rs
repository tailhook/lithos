extern crate serialize;
extern crate libc;
#[macro_use] extern crate log;
extern crate regex;

extern crate argparse;
extern crate quire;
#[macro_use] extern crate lithos;


use regex::Regex;
use std::old_io::stderr;
use std::old_io::IoError;
use std::old_io::ALL_PERMISSIONS;
use std::env::{set_exit_status};
use std::os::{self_exe_path};
use std::old_path::BytesContainer;
use std::str::FromStr;
use std::old_io::fs::{copy, rmdir_recursive, mkdir, readdir, rename};
use std::old_io::fs::{readlink, symlink, File};
use std::old_io::fs::PathExtensions;
use std::default::Default;
use std::old_io::process::{Command, InheritFd, ExitStatus};

use argparse::{ArgumentParser, Store, StoreOption, StoreTrue};
use quire::parse_config;

use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;
use lithos::signal;
use lithos::sha256::{Sha256,Digest};


fn hash_file(file: &Path) -> Result<String, IoError> {
    let mut hash = Sha256::new();
    // We assume that file is small enough so we don't care reading it
    // to memory
    hash.input(try!(try!(File::open(file)).read_to_end()).as_slice());
    return Ok(hash.result_str().as_slice().slice_to(8).to_string());
}


fn switch_config(master_cfg: Path, tree_name: String, config_file: Path,
    name_prefix: Option<String>)
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
        .arg("--alternate-config")
        .arg(&config_file)
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

    let config_fn = if let Some(prefix) = name_prefix {
        prefix.to_string() + try!(hash_file(&config_file)
            .map_err(|e| format!("Can't read file: {}", e))).as_slice() +
            ".yaml"
    } else {
        config_file.filename_str().unwrap().to_string()
    };
    let target_fn = Path::new(tree.config_file.dirname())
                    .join(config_fn.as_slice());
    if target_fn == tree.config_file {
        return Err(format!("Target config file ({:?}) \
            is the current file ({:?}). You must have unique \
            name for each new configuration",
            target_fn, tree.config_file));
    }
    let lnk = readlink(&tree.config_file);
    match lnk.as_ref().map(|f| f.dirname()) {
        Ok(b".") => {},
        _ => return Err(format!("The path {:?} must be a symlink \
            which points to the file at the same level of hierarchy.",
            tree.config_file)),
    };
    if lnk.unwrap().filename() == target_fn.filename() {
        warn!("Target config dir is the active one. Nothing to do.");
        return Ok(());
    }
    if target_fn.exists() {
        warn!("Target file {:?} exists. \
            Probably already copied. Skipping copying step.",
            target_fn);
    } else {
        info!("Seems everything nice. Copying");
        try!(copy(&config_file, &target_fn)
            .map_err(|e| format!("Error copying: {}", e)));
    }
    info!("Ok files are there. Making symlink");
    let symlink_fn = tree.config_file.with_filename(
        b".tmp.".to_vec() + tree.config_file.filename().unwrap());
    try!(symlink(&Path::new(config_fn.as_slice()), &symlink_fn)
        .map_err(|e| format!("Error symlinking dir: {}", e)));
    try!(rename(&symlink_fn, &tree.config_file)
        .map_err(|e| format!("Error replacing symlink: {}", e)));

    info!("Done. Sending SIGQUIT to lithos_tree");
    let pid_file = master.runtime_dir.join("master.pid");
    match File::open(&pid_file)
            .and_then(|mut f| f.read_to_string())
            .ok()
            .and_then(|s| FromStr::from_str(s.as_slice()).ok()) {
        Some(pid) if signal::is_process_alive(pid) => {
            signal::send_signal(pid, signal::SIGQUIT as isize);
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
    let mut name_prefix = None;
    let mut config_file = Path::new("");
    let mut tree_name = "".to_string();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Checks if lithos configuration is ok");
        ap.refer(&mut master_config)
          .add_option(&["--master"], Store,
            "Name of the master configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut verbose)
          .add_option(&["-v", "--verbose"], StoreTrue,
            "Verbose configuration");
        ap.refer(&mut name_prefix)
          .add_option(&["--hashed-name"], StoreOption, "
            Do not use last component of FILE as a name, but create an unique
            name based on the PREFIX and hash of the contents.
            ")
          .metavar("PREFIX");
        ap.refer(&mut tree_name)
          .add_argument("tree", Store,
            "Name of the tree which configuration will be switched for")
          .required()
          .metavar("NAME");
        ap.refer(&mut config_file)
          .add_argument("new_config", Store, "
            Name of the configuration directory to switch to. It doesn't
            have to be a directory inside `config-dir`, and it will be copied
            there. However, if directory with the same name exists in the
            `config-dir` it's assumed that it's already copied and will not
            be updated. Be sure to use unique directory for each deployment.
            ")
          .metavar("FILE")
          .required();
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match switch_config(master_config, tree_name, config_file, name_prefix) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(&mut stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
