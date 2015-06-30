extern crate rustc_serialize;
extern crate libc;
#[macro_use] extern crate log;
extern crate regex;
extern crate shaman;
extern crate argparse;
extern crate quire;
#[macro_use] extern crate lithos;


use std::env;
use std::io::{stderr, Read, Write};
use std::io::Error as IoError;
use std::process::exit;
use std::path::{Path, PathBuf};
use std::path::Component::Normal;
use std::str::FromStr;
use std::ffi::{OsStr, OsString};
use std::fs::{File};
use std::fs::{copy, rename, metadata, read_link};
use std::os::unix::fs::symlink;
use std::os::unix::ffi::OsStrExt;
use std::default::Default;
use std::process::{Command, Stdio};

use shaman::digest::Digest;
use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue};
use quire::parse_config;

use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;
use lithos::signal;


fn hash_file(file: &Path) -> Result<String, IoError> {
    let mut hash = shaman::sha2::Sha256::new();
    // We assume that file is small enough so we don't care reading it
    // to memory
    let mut buf = Vec::with_capacity(1024);
    try!(try!(File::open(file)).read_to_end(&mut buf));
    hash.input(&buf);
    Ok(hash.result_str())
}


fn switch_config(master_cfg: &Path, tree_name: String, config_file: &Path,
    name_prefix: Option<String>)
    -> Result<(), String>
{
    match Command::new(env::current_exe().unwrap()
                       .parent().unwrap().join("lithos_check"))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .arg("--config")
        .arg(&master_cfg)
        .arg("--tree")
        .arg(&tree_name)
        .arg("--alternate-config")
        .arg(&config_file)
        .output()
    {
        Ok(ref po) if po.status.code() == Some(0) => { }
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

    let config_fn: OsString = if let Some(prefix) = name_prefix {
        let hash = try!(hash_file(&config_file)
            .map_err(|e| format!("Can't read file: {}", e)));
        OsString::from(prefix + &hash + ".yaml")
    } else {
        config_file.file_name().unwrap().to_owned()
    };
    let target_fn = tree.config_file.parent().unwrap()
                    .join(&config_fn);
    if target_fn == tree.config_file {
        return Err(format!("Target config file ({:?}) \
            is the current file ({:?}). You must have unique \
            name for each new configuration",
            target_fn, tree.config_file));
    }
    let lnk = try!(read_link(&tree.config_file)
       .map_err(|e| format!("Can't read link {:?}: {}", tree.config_file, e)));
    match lnk.components().rev().nth(1) {
        Some(Normal(_)) => {},
        _ => return Err(format!("The path {:?} must be a symlink \
            which points to the file at the same level of hierarchy.",
            tree.config_file)),
    };
    if lnk.file_name() == target_fn.file_name() {
        warn!("Target config dir is the active one. Nothing to do.");
        return Ok(());
    }
    if metadata(&target_fn).is_ok() {
        warn!("Target file {:?} exists. \
            Probably already copied. Skipping copying step.",
            target_fn);
    } else {
        info!("Seems everything nice. Copying");
        try!(copy(&config_file, &target_fn)
            .map_err(|e| format!("Error copying: {}", e)));
    }
    info!("Ok files are there. Making symlink");
    let mut tmpname = OsStr::from_bytes(b".tmp.").to_owned();
    tmpname.push(tree.config_file.file_name().unwrap());
    let symlink_fn = tree.config_file.with_file_name(tmpname);
    try!(symlink(&Path::new(&config_fn), &symlink_fn)
        .map_err(|e| format!("Error symlinking dir: {}", e)));
    try!(rename(&symlink_fn, &tree.config_file)
        .map_err(|e| format!("Error replacing symlink: {}", e)));

    info!("Done. Sending SIGQUIT to lithos_tree");
    let pid_file = master.runtime_dir.join("master.pid");
    let mut buf = String::with_capacity(50);
    let read = File::open(&pid_file)
            .and_then(|mut f| f.read_to_string(&mut buf))
            .ok();
    match read.and_then(|_| FromStr::from_str(&buf).ok()) {
        Some(pid) if signal::is_process_alive(pid) => {
            signal::send_signal(pid, signal::SIGQUIT);
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
    let mut master_config = PathBuf::from("/etc/lithos.yaml");
    let mut verbose = false;
    let mut name_prefix = None;
    let mut config_file = PathBuf::from("");
    let mut tree_name = "".to_string();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Checks if lithos configuration is ok");
        ap.refer(&mut master_config)
          .add_option(&["--master"], Parse,
            "Name of the master configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut verbose)
          .add_option(&["-v", "--verbose"], StoreTrue,
            "Verbose configuration");
        ap.refer(&mut name_prefix)
          .add_option(&["--hashed-name"], ParseOption, "
            Do not use last component of FILE as a name, but create an unique
            name based on the PREFIX and hash of the contents.
            ")
          .metavar("PREFIX");
        ap.refer(&mut tree_name)
          .add_argument("tree", Parse,
            "Name of the tree which configuration will be switched for")
          .required()
          .metavar("NAME");
        ap.refer(&mut config_file)
          .add_argument("new_config", Parse, "
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
                exit(x);
            }
        }
    }
    match switch_config(&master_config, tree_name, &config_file, name_prefix)
    {
        Ok(()) => {
            exit(0);
        }
        Err(e) => {
            write!(&mut stderr(), "Fatal error: {}\n", e).unwrap();
            exit(1);
        }
    }
}
