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
use std::collections::TreeMap;

use argparse::{ArgumentParser, Store};
use quire::parse_config;

use lithos::tree_config::TreeConfig;
use lithos::child_config::ChildConfig;
use lithos::container_config::{ContainerConfig, Readonly, Persistent, Tmpfs};
use lithos::container_config::{parse_volume};
use lithos::container::{Command};
use lithos::mount::{bind_mount, mount_ro_recursive, mount_tmpfs, mount_private};
use lithos::mount::{mount_pseudo};
use lithos::monitor::{Monitor, Executor};
use lithos::utils::temporary_change_root;
use lithos::signal;


struct Target {
    name: Rc<String>,
    global: TreeConfig,
    child: ChildConfig,
    local: ContainerConfig,
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

        return cmd;
    }
}

fn map_dir(dir: &Path, dirs: &TreeMap<String, String>) -> Option<Path> {
    assert!(dir.is_absolute());
    for (prefix, real_dir) in dirs.iter() {
        let dir_prefix = Path::new(prefix.as_slice());
        if dir_prefix.is_ancestor_of(dir) {
            match dir.path_relative_from(&dir_prefix) {
                Some(tail) => {
                    assert!(!tail.is_absolute());
                    return Some(Path::new(real_dir.as_slice()).join(tail));
                }
                None => continue,
            }
        }
    }
    return None;
}

fn setup_filesystem(global: &TreeConfig, local: &ContainerConfig)
    -> Result<(), String>
{
    let mntdir = global.mount_dir.clone();
    assert!(mntdir.is_absolute());

    let mut volumes: Vec<(&String, &String)> = local.volumes.iter().collect();
    volumes.sort_by(|&(mp1, _), &(mp2, _)| mp1.len().cmp(&mp2.len()));

    for &(mp_str, volume_str) in volumes.iter() {
        let tmp_mp = Path::new(mp_str.as_slice());
        assert!(tmp_mp.is_absolute());  // should be checked earlier

        let dest = mntdir.join(
            tmp_mp.path_relative_from(&Path::new("/")).unwrap());
        match try_str!(parse_volume(volume_str.as_slice())) {
            Readonly(dir) => {
                let path = match map_dir(&dir, &global.readonly_paths).or_else(
                                 || map_dir(&dir, &global.writable_paths)) {
                    None => {
                        return Err(format!(concat!("Can't find volume for {},",
                            " probably missing entry in readonly-paths"),
                            dir.display()));
                    }
                    Some(path) => path,
                };
                try!(bind_mount(&path, &dest));
                try!(mount_ro_recursive(&dest));
            }
            Persistent(dir) => {
                let path = match map_dir(&dir, &global.writable_paths) {
                    None => {
                        return Err(format!(concat!("Can't find volume for {},",
                            " probably missing entry in writable-paths"),
                            dir.display()));
                    }
                    Some(path) => path,
                };
                try!(bind_mount(&path, &dest));
                try_str!(mount_private(&dest));
            }
            Tmpfs(opt) => {
                try!(mount_tmpfs(&dest, opt.as_slice()));
            }
        }
    }
    let devdir = mntdir.join("dev");
    try!(bind_mount(&Path::new(global.devfs_dir.as_slice()), &devdir));
    try!(mount_ro_recursive(&devdir));
    try!(mount_pseudo(&mntdir.join("sys"), "sysfs", "", true));
    try!(mount_pseudo(&mntdir.join("proc"), "proc", "", false));

    return Ok(());
}

fn read_local_config(global_cfg: &TreeConfig, child_cfg: &ChildConfig)
    -> Result<ContainerConfig, String>
{
    let image_path = global_cfg.image_dir.join(&child_cfg.image);
    try!(bind_mount(&image_path, &global_cfg.mount_dir));
    try!(mount_ro_recursive(&global_cfg.mount_dir));
    return temporary_change_root(&global_cfg.mount_dir, || {
        parse_config(&child_cfg.config,
            &*ContainerConfig::validator(), Default::default())
    });
}

fn run(name: String, global_cfg: Path, config: ChildConfig)
    -> Result<(), String>
{
    let global: TreeConfig = try_str!(parse_config(&global_cfg,
        &*TreeConfig::validator(), Default::default()));

    let local: ContainerConfig = try!(read_local_config(&global, &config));

    info!("[{:s}] Starting container", name);

    try!(setup_filesystem(&global, &local));

    let mut mon = Monitor::new(name.clone());
    let name = Rc::new(name + ".main");
    let timeo = Duration::milliseconds((local.restart_timeout*1000.) as i64);
    mon.add(name.clone(), box Target {
        name: name,
        global: global,
        child: config,
        local: local,
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
    };
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
        ap.refer(&mut config)
          .add_option(["--config"], box Store::<ChildConfig>,
            "JSON-serialized container configuration")
          .required()
          .metavar("JSON");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match run(name, global_config, config) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
