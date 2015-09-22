extern crate rustc_serialize;
extern crate libc;
#[macro_use] extern crate log;

extern crate argparse;
extern crate quire;
#[macro_use] extern crate lithos;

use std::rc::Rc;
use std::env;
use std::str::FromStr;
use std::io::{stderr, Write};
use std::path::{Path};
use std::default::Default;
use std::process::exit;

use quire::parse_config;
use unshare::{Command};

use lithos::signal;
use lithos::cgroup;
use lithos::utils::{in_range, check_mapping, in_mapping, change_root};
use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;
use lithos::container_config::{ContainerConfig};
use lithos::container_config::ContainerKind::Daemon;
use lithos::setup::{setup_filesystem, read_local_config, prepare_state_dir};
use lithos::setup::{init_logging};
use lithos::mount::{unmount, mount_private, bind_mount, mount_ro_recursive};
use lithos::limits::{set_fileno_limit};
use lithos_knot_options::Options;

mod lithos_knot_options;

struct Target {
    name: Rc<String>,
    local: ContainerConfig,
    args: Vec<String>,
}

impl Executor for Target {
    fn command(&self) -> Command
    {

        return cmd;
    }
    fn finish(&self) -> bool {
        return self.local.kind == Daemon
            && self.local.restart_process_only;
    }
}

fn run(options: Options) -> Result<(), String>
{
    let master: MasterConfig = try!(parse_config(&options.master_config,
        &*MasterConfig::validator(), Default::default())
        .map_err(|e| format!("Error reading master config: {}", e)));
    let tree_name = options.name[..].splitn(2, '/').next().unwrap();
    let tree: TreeConfig = try!(parse_config(
        &options.master_config.parent().unwrap()
         .join(&master.sandboxes_dir).join(tree_name.to_string() + ".yaml"),
        &*TreeConfig::validator(), Default::default())
        .map_err(|e| format!("Error reading tree config: {}", e)));

    let mut log_file;
    if let Some(ref fname) = tree.log_file {
        log_file = master.default_log_dir.join(fname);
    } else {
        log_file = master.default_log_dir.join(format!("{}.log", tree_name));
    }
    try!(init_logging(&log_file,
          options.log_level
            .or(tree.log_level.as_ref()
                .and_then(|x| FromStr::from_str(&x).ok()))
            .or_else(|| FromStr::from_str(&master.log_level).ok())
            .unwrap_or(log::LogLevel::Warn),
          options.log_stderr));

    try!(mount_private(&Path::new("/")));
    let image_path = tree.image_dir.join(&options.config.image);
    let mount_dir = master.runtime_dir.join(&master.mount_dir);
    try!(bind_mount(&image_path, &mount_dir));
    try!(mount_ro_recursive(&mount_dir));

    let local: ContainerConfig;
    local = try!(read_local_config(&mount_dir, &options.config));
    if local.kind != options.config.kind {
        return Err(format!("Container type mismatch {:?} != {:?}",
              local.kind, options.config.kind));
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

    info!("[{}] Starting container", options.name);

    let state_dir = &master.runtime_dir.join(&master.state_dir)
        .join(&options.name);
    try!(prepare_state_dir(state_dir, &local, &tree));
    try!(setup_filesystem(&master, &tree, &local, state_dir));
    if let Some(cgroup_parent) = master.cgroup_name {
        // Warning setting cgroup relative to it's own cgroup may not work
        // if we ever want to restart lithos_knot in-place
        let cgroups = try!(cgroup::ensure_in_group(
            &(cgroup_parent + "/" +
              &options.name.replace("/", ":") + ".scope"),
            &master.cgroup_controllers));
        cgroups.set_value(cgroup::Controller::Memory,
            "memory.limit_in_bytes",
            &format!("{}", local.memory_limit))
            .map_err(|e| error!("Error setting cgroup limit: {}", e)).ok();
        cgroups.set_value(cgroup::Controller::Cpu,
                "cpu.shares",
                &format!("{}", local.cpu_shares))
            .map_err(|e| error!("Error setting cgroup limit: {}", e)).ok();
    }

    let mount_dir = master.runtime_dir.join(&master.mount_dir);
    try!(change_root(&mount_dir, &mount_dir.join("tmp")));
    try!(unmount(Path::new("/tmp")));

    try!(set_fileno_limit(local.fileno_limit)
        .map_err(|e| format!("Error setting file limit: {}", e)));


    let mut cmd = Command::new(&local.executable);
    cmd.set_user(local.user_id, local.group_id);
    cmd.current_Dir(&local.workdir);

    // Should we propagate TERM?
    cmd.clear_env();
    cmd.env("TERM", env::var("TERM").unwrap_or("dumb".to_string()));
    cmd.update_env(local.environ);
    cmd.env("LITHOS_NAME", (*self.name).clone());

    cmd.args(&local.arguments);
    cmd.args(&options.args);
    if local.uid_map.len() > 0 || local.gid_map.len() > 0 {
        cmd.uid_map(&local.uid_map, &local.gid_map);
    }

    if let Some(ref path) = local.stdout_stderr_file {
        let f = OpenOptions::().append().open(path);
        cmd.stdout(f.as_raw_fd())
        cmd.stderr(f.as_raw_fd())
    }

    let mut mon = Monitor::new(options.name.clone());
    let name = Rc::new(options.name.clone() + ".main");
    let timeo = (local.restart_timeout*1000.) as i64;
    mon.add(name.clone(), Box::new(Target {
        name: name,
        local: local,
        args: options.args,
    }), timeo, None);
    mon.run();

    return Ok(());
}


fn main() {

    let options = match Options::parse_args() {
        Ok(options) => options,
        Err(x) => {
            exit(x);
        }
    };
    match run(options)
    {
        Ok(()) => {
            exit(0);
        }
        Err(e) => {
            write!(&mut stderr(), "Fatal error: {}\n", e).ok();
            error!("Fatal error: {}", e);
            exit(1);
        }
    }
}
