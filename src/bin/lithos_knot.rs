extern crate nix;
extern crate libc;
extern crate time;
extern crate quire;
extern crate signal;
extern crate unshare;
extern crate argparse;
extern crate syslog;
extern crate libmount;
extern crate rustc_serialize;
#[macro_use] extern crate log;
#[macro_use] extern crate lithos;

use std::env;
use std::str::FromStr;
use std::io::{stderr, Write};
use std::fs::OpenOptions;
use std::path::{Path};
use std::time::{Instant, Duration};
use std::thread::sleep;
use std::default::Default;
use std::process::exit;

use quire::parse_config;
use unshare::{Command, Stdio, reap_zombies};
use nix::sys::signal::{SIGINT, SIGTERM, SIGCHLD, SigNum};
use signal::trap::Trap;
use libmount::BindMount;

use lithos::cgroup;
use lithos::utils::{in_range, check_mapping, in_mapping, change_root};
use lithos::master_config::MasterConfig;
use lithos::sandbox_config::SandboxConfig;
use lithos::container_config::{ContainerConfig};
use lithos::container_config::ContainerKind::Daemon;
use lithos::setup::{setup_filesystem, read_local_config, prepare_state_dir};
use lithos::setup::{init_logging};
use lithos::mount::{unmount, mount_private, mount_ro_recursive};
use lithos::limits::{set_fileno_limit};
use lithos_knot_options::Options;

mod lithos_knot_options;

struct SignalIter<'a> {
    trap: &'a mut Trap,
    interrupt: bool,
}

impl<'a> SignalIter<'a> {
    fn new(trap: &mut Trap) -> SignalIter {
        SignalIter {
            trap: trap,
            interrupt: false,
        }
    }
    fn interrupt(&mut self) {
        self.interrupt = true;
    }
}

impl<'a> Iterator for SignalIter<'a> {
    type Item = SigNum;
    fn next(&mut self) -> Option<SigNum> {
        if self.interrupt {
            return self.trap.wait(Instant::now());
        } else {
            return self.trap.next();
        }
    }
}

fn run(options: Options) -> Result<(), String>
{
    let master: MasterConfig = try!(parse_config(&options.master_config,
        &MasterConfig::validator(), Default::default())
        .map_err(|e| format!("Error reading master config: {}", e)));
    let sandbox_name = options.name[..].splitn(2, '/').next().unwrap();
    let sandbox: SandboxConfig = try!(parse_config(
        &options.master_config.parent().unwrap()
         .join(&master.sandboxes_dir).join(sandbox_name.to_string() + ".yaml"),
        &SandboxConfig::validator(), Default::default())
        .map_err(|e| format!("Error reading sandbox config: {}", e)));

    let log_file;
    if let Some(ref fname) = sandbox.log_file {
        log_file = master.default_log_dir.join(fname);
    } else {
        log_file = master.default_log_dir.join(format!("{}.log", sandbox_name));
    }
    try!(init_logging(&master, &log_file,
        &format!("{}-{}", master.syslog_app_name, sandbox_name),
        options.log_stderr,
        options.log_level
            .or(sandbox.log_level.as_ref()
                .and_then(|x| FromStr::from_str(&x).ok()))
            .or_else(|| FromStr::from_str(&master.log_level).ok())
            .unwrap_or(log::LogLevel::Warn)));

    let stderr_path = master.stdio_log_dir
        .join(format!("{}.log", sandbox_name));
    let stderr_file = try!(OpenOptions::new()
                .create(true).append(true).write(true).open(&stderr_path)
                .map_err(|e| format!(
                    "Error opening stderr file {:?}: {}", stderr_path, e)));

    try!(mount_private(&Path::new("/")));
    let image_path = sandbox.image_dir.join(&options.config.image);
    let mount_dir = master.runtime_dir.join(&master.mount_dir);
    try!(BindMount::new(&image_path, &mount_dir).mount()
        .map_err(|e| e.to_string()));
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
        if !in_range(&sandbox.allow_users, local.user_id) {
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
        if !in_range(&sandbox.allow_groups, local.group_id) {
            return Err(format!("Group is not in allowed range (gid: {})",
                local.group_id));
        }
    }
    if !check_mapping(&sandbox.allow_users, &local.uid_map) {
        return Err("Bad uid mapping (probably doesn't match allow_users)"
            .to_string());
    }
    if !check_mapping(&sandbox.allow_groups, &local.gid_map) {
        return Err("Bad gid mapping (probably doesn't match allow_groups)"
            .to_string());
    }

    info!("[{}] Starting container", options.name);

    let state_dir = &master.runtime_dir.join(&master.state_dir)
        .join(&options.name);
    try!(prepare_state_dir(state_dir, &local, &sandbox));
    try!(setup_filesystem(&master, &sandbox, &local, state_dir));
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
    cmd.uid(local.user_id);
    cmd.gid(local.group_id);
    cmd.current_dir(&local.workdir);

    // Should we propagate TERM?
    cmd.env_clear();
    cmd.env("TERM", env::var("TERM").unwrap_or("dumb".to_string()));
    for (k, v) in local.environ.iter() {
        cmd.env(k, v);
    }
    cmd.env("LITHOS_NAME", &options.name);

    cmd.args(&local.arguments);
    cmd.args(&options.args);
    if sandbox.uid_map.len() > 0 || sandbox.gid_map.len() > 0 {
        cmd.set_id_maps(
            sandbox.uid_map.iter().map(|u| unshare::UidMap {
                inside_uid: u.inside,
                outside_uid: u.outside,
                count: u.count,
            }).collect(),
            sandbox.gid_map.iter().map(|g| unshare::GidMap {
                inside_gid: g.inside,
                outside_gid: g.outside,
                count: g.count,
            }).collect());
    } else if local.uid_map.len() > 0 || local.gid_map.len() > 0 {
        cmd.set_id_maps(
            local.uid_map.iter().map(|u| unshare::UidMap {
                inside_uid: u.inside,
                outside_uid: u.outside,
                count: u.count,
            }).collect(),
            local.gid_map.iter().map(|g| unshare::GidMap {
                inside_gid: g.inside,
                outside_gid: g.outside,
                count: g.count,
            }).collect());
    }
    let rtimeo = Duration::from_millis((local.restart_timeout*1000.0) as u64);

    let mut trap = Trap::trap(&[SIGINT, SIGTERM, SIGCHLD]);
    let mut should_exit = local.kind != Daemon || !local.restart_process_only;
    loop {
        let start = Instant::now();

        // Reopen file at each start
        if let Some(ref path) = local.stdout_stderr_file {
            let f = try!(OpenOptions::new()
                .create(true).append(true).write(true).open(path)
                .map_err(|e| format!(
                    "Error opening output file {:?}: {}", path, e)));
            cmd.stdout(try!(Stdio::dup_file(&f)
                .map_err(|e| format!("Duplicating file descriptor: {}", e))));
            cmd.stderr(Stdio::from_file(f));
        } else {
            cmd.stdout(try!(Stdio::dup_file(&stderr_file)
                .map_err(|e| format!("Duplicating file descriptor: {}", e))));
            cmd.stderr(try!(Stdio::dup_file(&stderr_file)
                .map_err(|e| format!("Duplicating file descriptor: {}", e))));
        };

        let child = try!(cmd.spawn().map_err(|e|
            format!("Error running {:?}: {}", options.name, e)));

        let mut iter = SignalIter::new(&mut trap);
        while let Some(signal) = iter.next() {
            match signal {
                SIGINT => {
                    // SIGINT is usually a Ctrl+C so it's sent to whole
                    // process group, so we don't need to do anything special
                    debug!("Received SIGINT. Waiting process to stop..");
                    should_exit = true;
                }
                SIGTERM => {
                    // SIGTERM is usually sent to a specific process so we
                    // forward it to children
                    debug!("Received SIGTERM signal, propagating");
                    should_exit = true;
                    child.signal(SIGTERM).ok();
                }
                SIGCHLD => {
                    for (pid, status) in reap_zombies() {
                        if pid == child.pid() {
                            error!("Process {:?} {}", options.name, status);
                            iter.interrupt();
                        }
                    }
                }
                _ => unreachable!(),
            }
        }

        if should_exit {
            break;
        }
        let left = rtimeo - (Instant::now() - start);
        if left > Duration::new(0, 0) {
            sleep(left);
        }
    }

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
