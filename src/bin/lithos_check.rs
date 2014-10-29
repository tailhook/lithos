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
use std::io::stderr;
use std::io::fs::readdir;
use std::os::{set_exit_status, self_exe_path};
use std::io::fs::PathExtensions;
use std::default::Default;

use argparse::{ArgumentParser, Store, StoreOption};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;
use lithos::child_config::ChildConfig;
use lithos::signal;


fn check_config(cfg: &TreeConfig) -> Result<(), String> {
    if !Path::new(cfg.devfs_dir.as_slice()).exists() {
        return Err(format!(
            "Devfs dir ({}) must exist and contain device nodes",
            cfg.devfs_dir));
    }
    return Ok(());
}

fn check(config_file: Path, config_dir: Option<Path>) -> Result<(), String> {
    let cfg: TreeConfig = try_str!(parse_config(&config_file,
        &*TreeConfig::validator(), Default::default()));

    try!(check_config(&cfg));
    let config_dir = config_dir.unwrap_or(cfg.config_dir);

    debug!("Checking child dir {}", config_dir.display());
    let dirlist = try_str!(readdir(&config_dir));
    for child_fn in dirlist.into_iter() {
        match (child_fn.filestem_str(), child_fn.extension_str()) {
            (Some(""), _) => continue,  // Hidden files
            (_, Some("yaml")) => {}
            _ => continue,  // Non-yaml, old, whatever, files
        }
        debug!("Checking {}", child_fn.display());
        let child_cfg: ChildConfig = try_str!(parse_config(&child_fn,
            &*ChildConfig::validator(), Default::default()));
        debug!("Opening config {}", child_fn.display());
        let config: ContainerConfig = try_str!(parse_config(
            &cfg.image_dir
                .join(child_cfg.image)
                .join(child_cfg.config.path_relative_from(
                    &Path::new("/")).unwrap()),
            &*ContainerConfig::validator(), Default::default()));
    }

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
    let mut config_dir = None;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Checks if lithos configuration is ok");
        ap.refer(&mut config_file)
          .add_option(["-C", "--config"], box Store::<Path>,
            "Name of the global configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut config_dir)
          .add_option(["-D", "--config-dir"], box StoreOption::<Path>,
            concat!("Name of the alterate directory with configs. ",
                    "Useful to test configuration directory before ",
                    "switching it to be primary one"))
          .metavar("DIR");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match check(config_file, config_dir) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
