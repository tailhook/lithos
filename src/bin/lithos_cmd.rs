#![feature(phase, macro_rules, if_let)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;

extern crate argparse;
extern crate quire;
#[phase(plugin, link)] extern crate lithos;
#[phase(plugin)] extern crate regex_macros;
extern crate regex;


use std::rc::Rc;
use std::os::{set_exit_status, self_exe_path, getenv};
use std::io::stderr;
use std::time::Duration;
use std::default::Default;
use serialize::json;
use libc::funcs::posix88::unistd::getpid;

use argparse::{ArgumentParser, Store, List};
use quire::parse_config;

use lithos::master_config::{MasterConfig, create_master_dirs};
use lithos::tree_config::TreeConfig;
use lithos::container_config::{Command};
use lithos::child_config::ChildConfig;
use lithos::container::{Command};
use lithos::monitor::{Monitor, Executor};
use lithos::signal;


struct Child {
    name: Rc<String>,
    master_file: Path,
    child_config_serialized: String,
    root_binary: Path,
    args: Vec<String>,
}

impl Executor for Child {
    fn command(&self) -> Command
    {
        let mut cmd = Command::new((*self.name).clone(), &self.root_binary);
        cmd.keep_sigmask();

        // Name is first here, so it's easily visible in ps
        cmd.arg("--name");
        cmd.arg(self.name.as_slice());

        cmd.arg("--master");
        cmd.arg(&self.master_file);
        cmd.arg("--config");
        cmd.arg(self.child_config_serialized.as_slice());
        cmd.set_env("TERM".to_string(),
                    getenv("TERM").unwrap_or("dumb".to_string()));
        if let Some(x) = getenv("RUST_LOG") {
            cmd.set_env("RUST_LOG".to_string(), x);
        }
        if let Some(x) = getenv("RUST_BACKTRACE") {
            cmd.set_env("RUST_BACKTRACE".to_string(), x);
        }
        cmd.args(self.args.as_slice());
        cmd.container();
        return cmd;
    }
    fn finish(&self) -> bool {
        return false;  // Do not restart
    }
}

fn run(master_cfg: Path, tree_name: String,
    command_name: String, args: Vec<String>)
    -> Result<(), String>
{
    let master: MasterConfig = try_str!(parse_config(&master_cfg,
        &*MasterConfig::validator(), Default::default()));
    try!(create_master_dirs(&master));

    if !regex!(r"^[\w-]+$").is_match(tree_name.as_slice()) {
        return Err(format!("Wrong tree name: {}", tree_name));
    }
    if !regex!(r"^[\w-]+$").is_match(command_name.as_slice()) {
        return Err(format!("Wrong ommand name: {}", command_name));
    }

    let tree: TreeConfig = try_str!(parse_config(
        &master.config_dir.join(tree_name + ".yaml"),
        &*TreeConfig::validator(), Default::default()));

    let child_fn = tree.config_dir.join(command_name + ".yaml".to_string());
    let child_cfg: ChildConfig = try_str!(parse_config(&child_fn,
        &*ChildConfig::validator(), Default::default()));

    debug!("Child fn: {}", child_fn.display());

    if child_cfg.kind != Command {
        return Err(format!("The target container is: {}", child_cfg.kind));
    }


    let name = Rc::new(format!("{}/cmd.{}.{}", tree_name,
        command_name, unsafe { getpid() }));
    info!("[{}] Running command with args {}", name, args);
    let mut mon = Monitor::new((*name).clone());
    let timeo = Duration::milliseconds(0);
    let mut args = args;
    args.insert(0, "--".to_string());
    mon.add(name.clone(), box Child {
        name: name,
        master_file: master_cfg,
        child_config_serialized: json::encode(&child_cfg),
        root_binary: self_exe_path().unwrap().join("lithos_knot"),
        args: args,
    }, timeo, None);
    mon.run();

    return Ok(());
}

fn main() {

    signal::block_all();

    let mut master_config = Path::new("/etc/lithos.yaml");
    let mut command_name = "".to_string();
    let mut tree_name = "".to_string();
    let mut args = vec!();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Runs tree of processes");
        ap.refer(&mut master_config)
          .add_option(["--master"], box Store::<Path>,
            "Name of the master configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut tree_name)
          .add_argument("subtree", box Store::<String>,
            "Name of the tree to run command for")
          .required();
        ap.refer(&mut command_name)
          .add_argument("name", box Store::<String>,
            "Name of the command to run")
          .required();
        ap.refer(&mut args)
          .add_argument("argument", box List::<String>,
            "Arguments for the command");
        ap.stop_on_first_argument(true);
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match run(master_config, tree_name, command_name, args) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
