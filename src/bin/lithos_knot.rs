#![feature(phase, macro_rules)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;

extern crate argparse;
extern crate quire;
#[phase(plugin, link)] extern crate lithos;

use std::rc::Rc;
use std::os::{set_exit_status, getenv};
use std::io::stderr;
use std::time::Duration;
use std::default::Default;

use argparse::{ArgumentParser, Store, List};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::child_config::ChildConfig;
use lithos::container_config::{ContainerConfig, Daemon};
use lithos::container::{Command};
use lithos::monitor::{Monitor, Executor};
use lithos::signal;
use lithos::setup::{setup_filesystem, read_local_config, prepare_state_dir};


struct Target {
    name: Rc<String>,
    global: TreeConfig,
    local: ContainerConfig,
    args: Vec<String>,
}

impl Executor for Target {
    fn command(&self) -> Command
    {
        let mut cmd = Command::new((*self.name).clone(),
            self.local.executable.as_slice());
        cmd.set_user_id(self.local.user_id);
        cmd.chroot(&self.global.mount_dir);
        cmd.set_workdir(&self.local.workdir);

        // Should we propagate TERM?
        cmd.set_env("TERM".to_string(),
                    getenv("TERM").unwrap_or("dumb".to_string()));
        cmd.update_env(self.local.environ.iter());
        cmd.set_env("LITHOS_NAME".to_string(), (*self.name).clone());

        cmd.args(self.local.arguments.as_slice());
        cmd.args(self.args.as_slice());

        return cmd;
    }
    fn finish(&self) -> bool {
        return self.local.kind == Daemon;
    }
}

fn run(name: String, global_cfg: Path, config: ChildConfig, args: Vec<String>)
    -> Result<(), String>
{
    let global: TreeConfig = try_str!(parse_config(&global_cfg,
        &*TreeConfig::validator(), Default::default()));

    // TODO(tailhook) clarify it: root is mounted in read_local_config
    let local: ContainerConfig = try!(read_local_config(&global, &config));
    if local.kind != config.kind {
        return Err(format!("Container type mismatch {} != {}",
              local.kind, config.kind));
    }

    info!("[{:s}] Starting container", name);

    let state_dir = &global.state_dir.join(name.as_slice());
    try!(prepare_state_dir(state_dir, &global, &local));
    try!(setup_filesystem(&global, &local, state_dir));

    let mut mon = Monitor::new(name.clone());
    let name = Rc::new(name + ".main");
    let timeo = Duration::milliseconds((local.restart_timeout*1000.) as i64);
    mon.add(name.clone(), box Target {
        name: name,
        global: global,
        local: local,
        args: args,
    }, timeo, None);
    mon.run();

    return Ok(());
}

fn main() {

    signal::block_all();

    let mut global_config = Path::new("/etc/lithos.yaml");
    let mut config = ChildConfig {
        instances: 0,
        image: Path::new(""),
        config: Path::new(""),
        kind: Daemon,
    };
    let mut name = "".to_string();
    let mut args = vec!();
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
        ap.refer(&mut config)
          .add_option(["--config"], box Store::<ChildConfig>,
            "JSON-serialized container configuration")
          .required()
          .metavar("JSON");
        ap.refer(&mut args)
          .add_argument("argument", box List::<String>,
            "Additional arguments for the command");
        ap.stop_on_first_argument(true);
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match run(name, global_config, config, args) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
