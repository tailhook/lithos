#![feature(phase, macro_rules)]

extern crate serialize;
#[phase(plugin, link)] extern crate log;

extern crate argparse;
extern crate quire;


use std::io::stderr;
use std::io::fs::readdir;
use std::os::set_exit_status;
use std::default::Default;

use argparse::{ArgumentParser, Store};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;

#[path="../mod.rs"]
mod lithos;

macro_rules! try_str {
    ($expr:expr) => {
        try!(($expr).map_err(|e| format!("{}: {}", stringify!($expr), e)))
    }
}


fn run(config_file: Path) -> Result<(), String> {
    let cfg: TreeConfig = try_str!(parse_config(&config_file,
        TreeConfig::validator(), Default::default()));

    let mut children: Vec<ContainerConfig> = Vec::new();
    for child_fn in try_str!(readdir(&cfg.config_dir)).iter() {
        match (child_fn.filestem_str(), child_fn.extension_str()) {
            (Some(""), _) => continue,  // Hidden files
            (_, Some(".yaml")) => {}
            _ => continue,  // Non-yaml, old, whatever, files
        }
        let child_cfg = try_str!(parse_config(&config_file,
            ContainerConfig::validator(), Default::default()));
        children.push(child_cfg);
    }

    return Ok(());
}


fn main() {
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
