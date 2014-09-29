
#![feature(phase, macro_rules)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;

extern crate argparse;
extern crate quire;

use std::os::{set_exit_status, getenv};
use std::io::stderr;
use std::default::Default;

use argparse::{ArgumentParser, Store};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;
use lithos::container::{Command};
use lithos::monitor::{Monitor, Executor};

#[path="../mod.rs"]
mod lithos;

struct Target {
    name: String,
    global: TreeConfig,
    local: ContainerConfig,
}

impl Executor for Target {
    fn command(&self) -> Command
    {
        let mut cmd = Command::new(self.local.executable.as_slice());

        // Should we propagate TERM?
        cmd.set_env("TERM".to_string(),
                    getenv("TERM").unwrap_or("dumb".to_string()));
        cmd.update_env(self.local.environ.iter());
        cmd.set_env("LITHOS_NAME".to_string(), self.name.clone());

        cmd.args(self.local.arguments.as_slice());

        return cmd;
    }
}

fn run(name: String, global_cfg: Path, local_cfg: Path) -> Result<(), String> {
    let global: TreeConfig = try_str!(parse_config(&global_cfg,
        TreeConfig::validator(), Default::default()));
    let local: ContainerConfig = try_str!(parse_config(&local_cfg,
        ContainerConfig::validator(), Default::default()));

    info!("[{:s}] Starting container", name);

    let mut mon = Monitor::new(name.clone());
    let name = name + ".main";
    mon.add(name.clone(), box Target {
        name: name,
        global: global,
        local: local,
    });
    mon.run();

    return Ok(());
}

fn main() {
    let mut global_config = Path::new("/etc/lithos.yaml");
    let mut container_config = Path::new("");
    let mut name = "".to_string();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Runs tree of processes");
        ap.refer(&mut name)
          .add_option(["--name"], box Store::<String>,
            "The process name");
        ap.refer(&mut global_config)
          .add_option(["--global-config"], box Store::<Path>,
            "Name of the global configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut container_config)
          .add_option(["--container-config"], box Store::<Path>,
            "Name of the container configuration file")
          .required()
          .metavar("FILE");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match run(name, global_config, container_config) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
