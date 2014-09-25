#![feature(phase, macro_rules)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;

extern crate argparse;
extern crate quire;


use std::io::stderr;
use std::io::fs::readdir;
use std::io::process::Command;
use std::os::{set_exit_status, self_exe_path};
use std::default::Default;
use std::collections::HashMap;

use argparse::{ArgumentParser, Store};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;
use lithos::monitor::Monitor;
use lithos::container::Command;

#[path="../mod.rs"]
mod lithos;

macro_rules! try_str {
    ($expr:expr) => {
        try!(($expr).map_err(|e| format!("{}: {}", stringify!($expr), e)))
    }
}

fn gen_command(name: &String, global_config: &Path,
    container_file: &Path, _container_config: &ContainerConfig) -> Command
{
    let mut cmd = Command::new(self_exe_path().unwrap().join("lithos_knot"));

    // Name is first here, so it's easily visible in ps
    cmd.arg("--name");
    cmd.arg(name.as_slice());

    cmd.arg("--global-config");
    cmd.arg(global_config);
    cmd.arg("--container-config");
    cmd.arg(container_file);
    return cmd;
}

fn run(config_file: Path) -> Result<(), String> {
    let cfg: TreeConfig = try_str!(parse_config(&config_file,
        TreeConfig::validator(), Default::default()));

    let mut children: HashMap<Path, ContainerConfig> = HashMap::new();
    for child_fn in try_str!(readdir(&cfg.config_dir)).move_iter() {
        match (child_fn.filestem_str(), child_fn.extension_str()) {
            (Some(""), _) => continue,  // Hidden files
            (_, Some(".yaml")) => {}
            _ => continue,  // Non-yaml, old, whatever, files
        }
        let child_cfg = try_str!(parse_config(&config_file,
            ContainerConfig::validator(), Default::default()));
        children.insert(child_fn, child_cfg);
    }

    let mut mon = Monitor::new();
    for (path, cfg) in children.iter() {
        for i in range(0, cfg.instances) {
            let name = format!("{}.{}", path.filestem_str(), i);
            mon.add(name, |name| gen_command(name, &config_file, path, cfg));
        }
    }
    mon.wait_all();

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
