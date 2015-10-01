extern crate rustc_serialize;
extern crate libc;
extern crate nix;
extern crate env_logger;
extern crate regex;
extern crate argparse;
extern crate quire;
#[macro_use] extern crate log;
extern crate lithos;


use std::env;
use std::io::{stderr, Read, Write};
use std::process::exit;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::fs::{File};
use std::fs::{copy, rename};
use std::default::Default;
use std::process::{Command, Stdio};

use argparse::{ArgumentParser, Parse, StoreTrue};
use quire::parse_config;
use nix::sys::signal::{SIGQUIT, kill};

use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;


fn switch_config(master_cfg: &Path, tree_name: String, config_file: &Path)
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
    let tree_fn = master_cfg.parent().unwrap()
        .join(&master.sandboxes_dir)
        .join(&(tree_name.clone() + ".yaml"));
    let tree: TreeConfig = match parse_config(&tree_fn,
        &*TreeConfig::validator(), Default::default())
    {
        Ok(cfg) => cfg,
        Err(e) => {
            return Err(format!("Can't parse tree config: {}", e));
        }
    };

    let target_fn = master_cfg.parent().unwrap()
        .join(&master.processes_dir)
        .join(tree.config_file.as_ref().unwrap_or(
            &PathBuf::from(&(tree_name.clone() + ".yaml"))));
    debug!("Target filename {:?}", target_fn);
    let tmp_filename = target_fn.with_file_name(
        &format!(".tmp.{}", tree_name));
    try!(copy(&config_file, &tmp_filename)
        .map_err(|e| format!("Error copying: {}", e)));
    try!(rename(&tmp_filename, &target_fn)
        .map_err(|e| format!("Error replacing file: {}", e)));

    info!("Done. Sending SIGQUIT to lithos_tree");
    let pid_file = master.runtime_dir.join("master.pid");
    let mut buf = String::with_capacity(50);
    let read = File::open(&pid_file)
            .and_then(|mut f| f.read_to_string(&mut buf))
            .ok();
    match read.and_then(|_| FromStr::from_str(buf[..].trim()).ok()) {
        Some(pid) if kill(pid, 0).is_ok() => {
            kill(pid, SIGQUIT)
            .map_err(|e| error!("Error sending QUIT to master: {:?}", e)).ok();
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
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let mut master_config = PathBuf::from("/etc/lithos/master.yaml");
    let mut verbose = false;
    let mut config_file = PathBuf::from("");
    let mut tree_name = "".to_string();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Checks if lithos configuration is ok");
        ap.refer(&mut master_config)
          .add_option(&["--master"], Parse,
            "Name of the master configuration file \
                (default /etc/lithos/master.yaml)")
          .metavar("FILE");
        ap.refer(&mut verbose)
          .add_option(&["-v", "--verbose"], StoreTrue,
            "Verbose configuration");
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
    match switch_config(&master_config, tree_name, &config_file)
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
