#![feature(phase, macro_rules)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;

extern crate argparse;
extern crate quire;


use std::rc::Rc;
use std::io::stderr;
use std::os::getenv;
use std::io::fs::readdir;
use std::os::{set_exit_status, self_exe_path};
use std::default::Default;
use std::collections::HashMap;

use argparse::{ArgumentParser, Store};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;
use lithos::monitor::{Monitor, Executor};
use lithos::container::Command;
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

fn run(config_file: Path) -> Result<(), String> {
    let cfg: TreeConfig = try_str!(parse_config(&config_file,
        TreeConfig::validator(), Default::default()));

    let mut children: HashMap<Path, ContainerConfig> = HashMap::new();
    debug!("Checking child dir {}", cfg.config_dir);//.display());
    for child_fn in try_str!(readdir(&Path::new(cfg.config_dir.as_slice()))).move_iter() {
        match (child_fn.filestem_str(), child_fn.extension_str()) {
            (Some(""), _) => continue,  // Hidden files
            (_, Some("yaml")) => {}
            _ => continue,  // Non-yaml, old, whatever, files
        }
        debug!("Adding {}", child_fn.display());
        let child_cfg = try_str!(parse_config(&child_fn,
            ContainerConfig::validator(), Default::default()));
        children.insert(child_fn, child_cfg);
    }

    let mut mon = Monitor::new("lithos-tree".to_string());
    let config_file = Rc::new(config_file);
    for (path, cfg) in children.move_iter() {
        let cfg = Rc::new(cfg);
        let path = Rc::new(path);
        let stem = path.filestem_str().unwrap();
        for i in range(0, cfg.instances) {
            let name = format!("{}.{}", stem, i);
            mon.add(name.clone(), box Child {
                name: name,
                global_config: config_file.clone(),
                container_file: path.clone(),
                container_config: cfg.clone(),
            });
        }
    }
    mon.run();

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
