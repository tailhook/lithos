use std::io::{ALL_PERMISSIONS, USER_RWX, GROUP_READ, OTHER_READ};
use std::io::fs::{File, copy, chmod, mkdir_recursive, chown};
use std::io::fs::PathExtensions;
use std::default::Default;
use std::collections::TreeMap;

use quire::parse_config;

use super::mount::{bind_mount, mount_ro_recursive, mount_tmpfs};
use super::mount::{mount_pseudo};
use super::network::{get_host_ip, get_host_name};
use super::master_config::MasterConfig;
use super::tree_config::TreeConfig;
use super::container_config::{ContainerConfig, Readonly, Persistent, Tmpfs};
use super::container_config::{Statedir};
use super::container_config::{parse_volume};
use super::child_config::ChildConfig;
use super::utils::{temporary_change_root, clean_dir};
use super::cgroup;


fn map_dir(dir: &Path, dirs: &TreeMap<Path, Path>) -> Option<Path> {
    assert!(dir.is_absolute());
    for (prefix, real_dir) in dirs.iter() {
        if prefix.is_ancestor_of(dir) {
            match dir.path_relative_from(prefix) {
                Some(tail) => {
                    assert!(!tail.is_absolute());
                    return Some(real_dir.join(tail));
                }
                None => continue,
            }
        }
    }
    return None;
}

pub fn setup_filesystem(master: &MasterConfig, tree: &TreeConfig,
    local: &ContainerConfig, state_dir: &Path)
    -> Result<(), String>
{
    let root = Path::new("/");
    let mntdir = master.runtime_dir.join(&master.mount_dir);
    assert!(mntdir.is_absolute());

    let mut volumes: Vec<(&String, &String)> = local.volumes.iter().collect();
    volumes.sort_by(|&(mp1, _), &(mp2, _)| mp1.len().cmp(&mp2.len()));

    for &(mp_str, volume_str) in volumes.iter() {
        let tmp_mp = Path::new(mp_str.as_slice());
        assert!(tmp_mp.is_absolute());  // should be checked earlier

        let dest = mntdir.join(tmp_mp.path_relative_from(&root).unwrap());
        match try_str!(parse_volume(volume_str.as_slice())) {
            Readonly(dir) => {
                let path = match map_dir(&dir, &tree.readonly_paths).or_else(
                                 || map_dir(&dir, &tree.writable_paths)) {
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
                let path = match map_dir(&dir, &tree.writable_paths) {
                    None => {
                        return Err(format!(concat!("Can't find volume for {},",
                            " probably missing entry in writable-paths"),
                            dir.display()));
                    }
                    Some(path) => path,
                };
                // TODO(tailhook) make it parametrized
                if !path.exists() {
                    try_str!(mkdir_recursive(&path, ALL_PERMISSIONS));
                    // TODO(tailhook) map actual user
                    try_str!(chown(&path, local.user_id as int, -1));
                }
                try!(bind_mount(&path, &dest));
            }
            Tmpfs(opt) => {
                try!(mount_tmpfs(&dest, opt.as_slice()));
            }
            Statedir(dir) => {
                let relative_dir = dir.path_relative_from(&root).unwrap();
                try!(bind_mount(
                    &state_dir.join(relative_dir),
                    &dest));
            }
        }
    }
    let devdir = mntdir.join("dev");
    try!(bind_mount(&master.devfs_dir, &devdir));
    try!(mount_ro_recursive(&devdir));
    try!(mount_pseudo(&mntdir.join("sys"), "sysfs", "", true));
    try!(mount_pseudo(&mntdir.join("proc"), "proc", "", false));

    return Ok(());
}

pub fn prepare_state_dir(dir: &Path, local: &ContainerConfig,
    tree: &TreeConfig)
    -> Result<(), String>
{
    // TODO(tailhook) chown files
    if !dir.exists() {
        try!(mkdir_recursive(dir, ALL_PERMISSIONS)
            .map_err(|e| format!("Couldn't create state directory: {}", e)));
    }
    if local.resolv_conf.copy_from_host {
        try_str!(copy(&Path::new("/etc/resolv.conf"),
                      &dir.join("resolv.conf")));
    }
    if local.hosts_file.localhost || local.hosts_file.public_hostname
        || tree.additional_hosts.len() > 0
    {
        let fname = dir.join("hosts");
        let mut file = try_str!(File::create(&fname));
        if local.hosts_file.localhost {
            try_str!(file.write_str(
                "127.0.0.1 localhost.localdomain localhost\n"));
        }
        if local.hosts_file.public_hostname {
            try_str!(writeln!(file, "{} {}",
                try_str!(get_host_ip()),
                try_str!(get_host_name())));
        }
        for (ref host, ref ip) in tree.additional_hosts.iter() {
            try_str!(writeln!(file, "{} {}", ip, host));
        }
        try_str!(chmod(&fname, USER_RWX|GROUP_READ|OTHER_READ));
    }
    return Ok(());
}

pub fn read_local_config(root: &Path, child_cfg: &ChildConfig)
    -> Result<ContainerConfig, String>
{
    return temporary_change_root(root, || {
        parse_config(&Path::new(child_cfg.config.as_slice()),
            &*ContainerConfig::validator(), Default::default())
    });
}

pub fn clean_child(name: &String, master: &MasterConfig) {
    let st_dir = master.runtime_dir
        .join(&master.state_dir).join(name.as_slice());
    clean_dir(&st_dir, true)
        .map_err(|e| error!("Error removing state dir for {}: {}", name, e))
        .ok();
    if let Some(ref master_grp) = master.cgroup_name {
        let cgname = name.replace("/", ":") + ".scope";
        cgroup::remove_child_cgroup(cgname.as_slice(), master_grp,
                                    &master.cgroup_controllers)
            .map_err(|e| error!("Error removing cgroup: {}", e))
            .ok();
    }
}
