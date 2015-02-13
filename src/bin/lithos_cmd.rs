extern crate serialize;
extern crate libc;
#[macro_use] extern crate log;
extern crate regex;

extern crate argparse;
extern crate quire;
#[macro_use] extern crate lithos;


use regex::Regex;
use std::rc::Rc;
use std::os::{set_exit_status, self_exe_path, getenv};
use std::io::stderr;
use std::time::Duration;
use std::default::Default;
use std::collections::BTreeMap;
use serialize::json;
use libc::funcs::posix88::unistd::getpid;

use argparse::{ArgumentParser, Store, List};
use quire::parse_config;

use lithos::setup::clean_child;
use lithos::master_config::{MasterConfig, create_master_dirs};
use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerKind::{Command};
use lithos::child_config::ChildConfig;
use lithos::container::{Command};
use lithos::monitor::{Monitor, Executor};
use lithos::signal;


struct Child<'a> {
    name: Rc<String>,
    master_file: Path,
    master_config: &'a MasterConfig,
    child_config_serialized: String,
    root_binary: Path,
    args: Vec<String>,
}

impl<'a> Executor for Child<'a> {
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
        clean_child(&*self.name, self.master_config);
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

    if !Regex::new(r"^[\w-]+$").unwrap().is_match(tree_name.as_slice()) {
        return Err(format!("Wrong tree name: {}", tree_name));
    }
    if !Regex::new(r"^[\w-]+$").unwrap().is_match(command_name.as_slice()) {
        return Err(format!("Wrong command name: {}", command_name));
    }

    let tree: TreeConfig = try_str!(parse_config(
        &master.config_dir.join(tree_name.clone() + ".yaml"),
        &*TreeConfig::validator(), Default::default()));

    debug!("Children config {:?}", tree.config_file);
    let tree_children: BTreeMap<String, ChildConfig>;
    tree_children = try_str!(parse_config(&tree.config_file,
        &*ChildConfig::mapping_validator(), Default::default()));
    let child_cfg = try!(tree_children.get(&command_name)
        .ok_or(format!("Command {:?} not found", command_name)));



    if child_cfg.kind != Command {
        return Err(format!("The target container is: {:?}", child_cfg.kind));
    }


    let name = Rc::new(format!("{}/cmd.{}.{}", tree_name,
        command_name, unsafe { getpid() }));
    info!("[{}] Running command with args {:?}", name, args);
    let mut mon = Monitor::new((*name).clone());
    let timeo = Duration::milliseconds(0);
    let mut args = args;
    args.insert(0, "--".to_string());
    mon.add(name.clone(), Box::new(Child {
        name: name,
        master_file: master_cfg,
        master_config: &master,
        child_config_serialized: json::encode(&child_cfg),
        root_binary: self_exe_path().unwrap().join("lithos_knot"),
        args: args,
    }), timeo, None);
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
          .add_option(&["--master"], Box::new(Store::<Path>),
            "Name of the master configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut tree_name)
          .add_argument("subtree", Box::new(Store::<String>),
            "Name of the tree to run command for")
          .required();
        ap.refer(&mut command_name)
          .add_argument("name", Box::new(Store::<String>),
            "Name of the command to run")
          .required();
        ap.refer(&mut args)
          .add_argument("argument", Box::new(List::<String>),
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
            (write!(&mut stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
