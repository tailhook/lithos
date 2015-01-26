#![feature(phase, macro_rules, if_let)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;

extern crate argparse;
extern crate quire;
#[phase(plugin, link)] extern crate lithos;

use std::rc::Rc;
use std::os::{set_exit_status, getenv, args};
use std::io::stdio::{stdout, stderr};
use std::time::Duration;
use std::default::Default;

use argparse::{ArgumentParser, Store, List};
use quire::parse_config;

use lithos::signal;
use lithos::cgroup;
use lithos::utils::{in_range, check_mapping, in_mapping, change_root};
use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;
use lithos::child_config::ChildConfig;
use lithos::container_config::{ContainerConfig, Daemon};
use lithos::container::{Command};
use lithos::monitor::{Monitor, Executor};
use lithos::setup::{setup_filesystem, read_local_config, prepare_state_dir};
use lithos::mount::{unmount, mount_private, bind_mount, mount_ro_recursive};
use lithos::limits::{set_fileno_limit};


struct Target {
    name: Rc<String>,
    local: ContainerConfig,
    args: Vec<String>,
}

impl Executor for Target {
    fn command(&self) -> Command
    {
        let mut cmd = Command::new((*self.name).clone(),
            self.local.executable.as_slice());
        cmd.set_user(self.local.user_id, self.local.group_id);
        cmd.set_workdir(&self.local.workdir);

        // Should we propagate TERM?
        cmd.set_env("TERM".to_string(),
                    getenv("TERM").unwrap_or("dumb".to_string()));
        cmd.update_env(self.local.environ.iter());
        cmd.set_env("LITHOS_NAME".to_string(), (*self.name).clone());
        if let Some(ref path) = self.local.stdout_stderr_file {
            cmd.set_output(path);
        }

        cmd.args(self.local.arguments.as_slice());
        cmd.args(self.args.as_slice());
        if self.local.uid_map.len() > 0 || self.local.gid_map.len() > 0 {
            cmd.user_ns(&self.local.uid_map, &self.local.gid_map);
        }

        return cmd;
    }
    fn finish(&self) -> bool {
        return self.local.kind == Daemon;
    }
}

fn run(name: String, master_file: Path, config: ChildConfig, args: Vec<String>)
    -> Result<(), String>
{
    let master: MasterConfig = try_str!(parse_config(&master_file,
        &*MasterConfig::validator(), Default::default()));
    let tree_name = name.as_slice().splitn(1, '/').next().unwrap();
    let tree: TreeConfig = try_str!(parse_config(
        &master.config_dir.join(tree_name.to_string() + ".yaml"),
        &*TreeConfig::validator(), Default::default()));

    try!(mount_private(&Path::new("/")));
    let image_path = tree.image_dir.join(config.image.as_slice());
    let mount_dir = master.runtime_dir.join(&master.mount_dir);
    try!(bind_mount(&image_path, &mount_dir));
    try!(mount_ro_recursive(&mount_dir));

    let local: ContainerConfig;
    local = try!(read_local_config(&mount_dir, &config));
    if local.kind != config.kind {
        return Err(format!("Container type mismatch {} != {}",
              local.kind, config.kind));
    }
    if local.uid_map.len() > 0 {
        if !in_mapping(&local.uid_map, local.user_id) {
            return Err(format!("User is not in mapped range (uid: {})",
                local.user_id));
        }
    } else {
        if !in_range(&tree.allow_users, local.user_id) {
            return Err(format!("User is not in allowed range (uid: {})",
                local.user_id));
        }
    }
    if local.gid_map.len() > 0 {
        if !in_mapping(&local.gid_map, local.group_id) {
            return Err(format!("Group is not in mapped range (gid: {})",
                local.user_id));
        }
    } else {
        if !in_range(&tree.allow_groups, local.group_id) {
            return Err(format!("Group is not in allowed range (gid: {})",
                local.group_id));
        }
    }
    if !check_mapping(&tree.allow_users, &local.uid_map) {
        return Err("Bad uid mapping (probably doesn't match allow_users)"
            .to_string());
    }
    if !check_mapping(&tree.allow_groups, &local.gid_map) {
        return Err("Bad gid mapping (probably doesn't match allow_groups)"
            .to_string());
    }

    info!("[{:s}] Starting container", name);

    let state_dir = &master.runtime_dir.join(&master.state_dir)
        .join(name.as_slice());
    try!(prepare_state_dir(state_dir, &local, &tree));
    try!(setup_filesystem(&master, &tree, &local, state_dir));
    if let Some(cgroup_parent) = master.cgroup_name {
        // Warning setting cgroup relative to it's own cgroup may not work
        // if we ever want to restart lithos_knot in-place
        try!(cgroup::ensure_in_group(
            &(cgroup_parent + "/" + name.replace("/", ":") + ".scope"),
            &master.cgroup_controllers));
    }

    let mount_dir = master.runtime_dir.join(&master.mount_dir);
    try!(change_root(&mount_dir, &mount_dir.join("tmp")));
    try!(unmount(&Path::new("/tmp")));

    try_str!(set_fileno_limit(local.fileno_limit));

    let mut mon = Monitor::new(name.clone());
    let name = Rc::new(name + ".main");
    let timeo = Duration::milliseconds((local.restart_timeout*1000.) as i64);
    mon.add(name.clone(), box Target {
        name: name,
        local: local,
        args: args,
    }, timeo, None);
    mon.run();

    return Ok(());
}

pub struct Options {
    pub master_config: Path,
    pub config: ChildConfig,
    pub name: String,
    pub args: Vec<String>,
}

impl Options {
    pub fn parse_args() -> Result<Options, int> {
        Options::parse_specific_args(args(), &mut stdout(), &mut stderr())
    }
    pub fn parse_specific_args(args: Vec<String>,
        stdout: &mut Writer, stderr: &mut Writer)
        -> Result<Options, int>
    {
        let mut options = Options {
            master_config: Path::new("/etc/lithos.yaml"),
            config: ChildConfig {
                instances: 0,
                image: "".to_string(),
                config: "".to_string(),
                kind: Daemon,
            },
            name: "".to_string(),
            args: vec!(),
        };
        let mut ap = ArgumentParser::new();
        ap.set_description("Runs tree of processes");
        ap.refer(&mut options.name)
          .add_option(["--name"], box Store::<String>,
            "The process name");
        ap.refer(&mut options.master_config)
          .add_option(["--master"], box Store::<Path>,
            "Name of the master configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut options.config)
          .add_option(["--config"], box Store::<ChildConfig>,
            "JSON-serialized container configuration")
          .required()
          .metavar("JSON");
        ap.refer(&mut options.args)
          .add_argument("argument", box List::<String>,
            "Additional arguments for the command");
        ap.stop_on_first_argument(true);
        match ap.parse(args, stdout, stderr) {
            Ok(()) => Ok(options),
            Err(x) => Err(x),
        }
    }
}

fn main() {

    signal::block_all();

    let options = match Options::parse_args() {
        Ok(options) => options,
        Err(x) => {
            set_exit_status(x);
            return;
        }
    };
    match run(options.name, options.master_config,
              options.config, options.args)
    {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
