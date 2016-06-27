use std::io;
use std::io::{Write, stderr};
use std::fs::{File};
use std::fs::{create_dir_all, copy, metadata};
use std::path::{Path, PathBuf};
use std::default::Default;
use std::collections::BTreeMap;

use log;
use fern;
use syslog;
use time;
use quire::parse_config;
use libmount::{self, BindMount};

use super::mount::{mount_ro_recursive};
use super::mount::{mount_pseudo};
use super::network::{get_host_ip, get_host_name};
use super::master_config::MasterConfig;
use super::sandbox_config::SandboxConfig;
use super::container_config::{ContainerConfig, Volume};
use super::container_config::Volume::{Statedir, Readonly, Persistent, Tmpfs};
use super::child_config::ChildConfig;
use super::utils::{temporary_change_root, clean_dir};
use super::utils::{set_file_mode, set_file_owner};
use super::utils::{relative};
use super::cgroup;


fn map_dir(dir: &Path, dirs: &BTreeMap<PathBuf, PathBuf>) -> Option<PathBuf> {
    assert!(dir.is_absolute());
    for (prefix, real_dir) in dirs.iter() {
        if dir.starts_with(prefix) {
            return Some(real_dir.join(relative(dir, prefix)));
        }
    }
    return None;
}

pub fn setup_filesystem(master: &MasterConfig, tree: &SandboxConfig,
    local: &ContainerConfig, state_dir: &Path)
    -> Result<(), String>
{
    let root = PathBuf::from("/");
    let mntdir = master.runtime_dir.join(&master.mount_dir);
    assert!(mntdir.is_absolute());

    let mut volumes: Vec<(&String, &Volume)> = local.volumes.iter().collect();
    volumes.sort_by(|&(mp1, _), &(mp2, _)| mp1.len().cmp(&mp2.len()));

    for &(mp_str, volume) in volumes.iter() {
        let tmp_mp = PathBuf::from(&mp_str[..]);
        assert!(tmp_mp.is_absolute());  // should be checked earlier

        let dest = mntdir.join(relative(&tmp_mp, &root));
        match volume {
            &Readonly(ref dir) => {
                let path = match map_dir(dir, &tree.readonly_paths).or_else(
                                 || map_dir(dir, &tree.writable_paths)) {
                    None => {
                        return Err(format!(concat!("Can't find volume for {},",
                            " probably missing entry in readonly-paths"),
                            dir.display()));
                    }
                    Some(path) => path,
                };
                try!(BindMount::new(&path, &dest).mount()
                    .map_err(|x| x.to_string()));
                try!(mount_ro_recursive(&dest));
            }
            &Persistent(ref opt) => {
                let path = match map_dir(&opt.path, &tree.writable_paths) {
                    None => {
                        return Err(format!("Can't find volume for {:?}, \
                            probably missing entry in writable-paths",
                            opt.path));
                    }
                    Some(path) => path,
                };
                if metadata(&path).is_err() {
                    if opt.mkdir {
                        try!(create_dir_all(&path)
                            .map_err(|e| format!("Error creating \
                                persistent volume: {}", e)));
                        let user = try!(local.map_uid(opt.user)
                            .ok_or(format!("Non-mapped user {} for volume {}",
                                opt.user, mp_str)));
                        let group = try!(local.map_gid(opt.group)
                            .ok_or(format!("Non-mapped group {} for volume {}",
                                opt.group, mp_str)));
                        try!(set_file_owner(&path, user, group)
                            .map_err(|e| format!("Error chowning \
                                persistent volume: {}", e)));
                        try!(set_file_mode(&path, opt.mode)
                            .map_err(|e| format!("Can't chmod persistent \
                                volume: {}", e)));
                    }
                }
                try!(BindMount::new(&path, &dest).mount()
                    .map_err(|x| x.to_string()));
            }
            &Tmpfs(ref opt) => {
                try!(libmount::Tmpfs::new(&dest)
                    .size_bytes(opt.size).mode(opt.mode)
                    .mount().map_err(|e| e.to_string()));
            }
            &Statedir(ref opt) => {
                let relative_dir = relative(&opt.path, &root);
                let dir = state_dir.join(&relative_dir);
                if Path::new(&relative_dir) != Path::new(".") {
                    try!(create_dir_all(&dir)
                        .map_err(|e| format!("Error creating \
                            persistent volume: {}", e)));
                    let user = try!(local.map_uid(opt.user)
                        .ok_or(format!("Non-mapped user {} for volume {}",
                            opt.user, mp_str)));
                    let group = try!(local.map_gid(opt.group)
                        .ok_or(format!("Non-mapped group {} for volume {}",
                            opt.group, mp_str)));
                    try!(set_file_owner(&dir, user, group)
                        .map_err(|e| format!("Error chowning \
                            persistent volume: {}", e)));
                    try!(set_file_mode(&dir, opt.mode)
                        .map_err(|e| format!("Can't chmod persistent \
                            volume: {}", e)));
                }
                try!(BindMount::new(&dir, &dest).mount()
                    .map_err(|x| x.to_string()));
            }
        }
    }
    let devdir = mntdir.join("dev");
    try!(BindMount::new(&master.devfs_dir, &devdir).mount()
        .map_err(|x| x.to_string()));
    try!(mount_ro_recursive(&devdir));
    try!(mount_pseudo(&mntdir.join("dev/pts"),
        "devpts", "newinstance", false));
    try!(mount_pseudo(&mntdir.join("sys"), "sysfs", "", true));
    try!(mount_pseudo(&mntdir.join("proc"), "proc", "", false));

    return Ok(());
}

pub fn prepare_state_dir(dir: &Path, local: &ContainerConfig,
    tree: &SandboxConfig)
    -> Result<(), String>
{
    // TODO(tailhook) chown files
    if metadata(dir).is_err() {
        try!(create_dir_all(dir)
            .map_err(|e| format!("Couldn't create state directory: {}", e)));
        try!(set_file_mode(dir, 0o1777)
            .map_err(|e| format!("Couldn't set chmod for state dir: {}", e)));
    }
    if local.resolv_conf.copy_from_host {
        try!(copy(&Path::new("/etc/resolv.conf"), &dir.join("resolv.conf"))
            .map_err(|e| format!("State dir: {}", e)));
    }
    let copy_hosts = local.hosts_file.copy_from_host;
    let add_localhost = local.hosts_file.localhost.unwrap_or(!copy_hosts);
    let add_hostname = local.hosts_file.public_hostname.unwrap_or(!copy_hosts);
    if add_localhost || add_hostname || copy_hosts
        || tree.additional_hosts.len() > 0
    {
        let fname = dir.join("hosts");
        try!(File::create(&fname)
            .and_then(|mut file| {
                if copy_hosts {
                    let mut source = try!(File::open("/etc/hosts"));
                    try!(io::copy(&mut source, &mut file));
                    // In case file has no newline at the end and we are
                    // going to add some records
                    try!(file.write_all(b"\n"));
                }
                if add_localhost {
                    try!(file.write_all(
                        "127.0.0.1 localhost.localdomain localhost\n"
                        .as_bytes()));
                }
                if add_hostname {
                    try!(writeln!(&mut file, "{} {}",
                        try!(get_host_ip()),
                        try!(get_host_name())));
                }
                for (ref host, ref ip) in tree.additional_hosts.iter() {
                    try!(writeln!(&mut file, "{} {}", ip, host));
                }
                Ok(())
            })
            .map_err(|e| format!("Error writing hosts: {}", e)));
        set_file_mode(&fname, 0o644).ok(); // TODO(tailhook) check error?
    }
    return Ok(());
}

pub fn read_local_config(root: &Path, child_cfg: &ChildConfig)
    -> Result<ContainerConfig, String>
{
    return temporary_change_root(root, || {
        parse_config(&Path::new(&child_cfg.config),
            &ContainerConfig::validator(), Default::default())
    });
}

pub fn clean_child(name: &str, master: &MasterConfig, temporary: bool) {
    let st_dir = master.runtime_dir
        .join(&master.state_dir).join(name);
    clean_dir(&st_dir, true)
        .map_err(|e| error!("Error removing state dir for {}: {}", name, e))
        .ok();
    if !temporary {
        // If shutdown is temporary (i.e. process failed and we are going to
        // restart it shortly), we don't remove cgroups. Because removing
        // them triggers the following bug in the memory cgroup controller:
        //
        // https://lkml.org/lkml/2016/6/15/1135
        //
        // I mean this is still not fixed in linux 4.6, so while we may be
        // able to get rid of this. But this won't gonna happen in 2-3 years :(
        //
        // Anyway it's possible that we don't need this in the new (unified)
        // cgroup hierarhy which is already there in 4.5, but we don't support
        // it yet.
        if let Some(ref master_grp) = master.cgroup_name {
            let cgname = name.replace("/", ":") + ".scope";
            cgroup::remove_child_cgroup(&cgname, master_grp,
                                        &master.cgroup_controllers)
                .map_err(|e| error!("Error removing cgroup: {}", e))
                .ok();
        }
    }
}

pub fn init_logging(cfg: &MasterConfig, suffix: &Path, name: &str,
    log_stderr: bool, level: log::LogLevel)
    -> Result<(), String>
{
    let sysfac = cfg.syslog_facility.as_ref()
        .and_then(|v| v.parse()
            .map_err(|_| writeln!(&mut stderr(),
                "Can't parse syslog facility: {:?}. Syslog is disabled.", v))
            .ok());
    if let Some(facility) = sysfac {
        syslog::init(facility, level.to_log_level_filter(), Some(&name))
        .map_err(|e| format!("Can't initialize logging: {}", e))
    } else {
        let path = cfg.default_log_dir.join(suffix);
        let mut output = vec![
            fern::OutputConfig::file(&path)
            ];
        if log_stderr {
            output.push(fern::OutputConfig::stderr());
        }
        let logger_config = fern::DispatchConfig {
            format: Box::new(|msg: &str, level: &log::LogLevel,
                              location: &log::LogLocation| {
                if *level >= log::LogLevel::Debug {
                    format!("[{}][{}]{}:{}: {}",
                        time::now_utc().rfc3339(),
                        level, location.file(), location.line(),
                        msg)
                } else {
                    format!("[{}][{}] {}",
                        time::now_utc().rfc3339(),
                        level, msg)
                }
            }),
            output: output,
            level: level.to_log_level_filter(),
        };
        fern::init_global_logger(logger_config, log::LogLevelFilter::Trace)
            .map_err(|e| format!("Can't initialize logging: {}", e))
    }
}
