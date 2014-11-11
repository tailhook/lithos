use std::io::FilePermission;
use std::io::fs::{File, copy, mkdir, chmod, mkdir_recursive, chown};
use std::io::fs::PathExtensions;
use std::default::Default;
use std::collections::TreeMap;

use quire::parse_config;

use super::mount::{bind_mount, mount_ro_recursive, mount_tmpfs, mount_private};
use super::mount::{mount_pseudo};
use super::network::{get_host_ip, get_host_name};
use super::tree_config::TreeConfig;
use super::container_config::{ContainerConfig, Readonly, Persistent, Tmpfs};
use super::container_config::{Statedir};
use super::container_config::{parse_volume};
use super::child_config::ChildConfig;
use super::utils::temporary_change_root;


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

pub fn setup_filesystem(global: &TreeConfig, local: &ContainerConfig,
    state_dir: &Path)
    -> Result<(), String>
{
    let root = Path::new("/");
    let mntdir = global.mount_dir.clone();
    assert!(mntdir.is_absolute());

    let mut volumes: Vec<(&String, &String)> = local.volumes.iter().collect();
    volumes.sort_by(|&(mp1, _), &(mp2, _)| mp1.len().cmp(&mp2.len()));

    for &(mp_str, volume_str) in volumes.iter() {
        let tmp_mp = Path::new(mp_str.as_slice());
        assert!(tmp_mp.is_absolute());  // should be checked earlier

        let dest = mntdir.join(tmp_mp.path_relative_from(&root).unwrap());
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
                // TODO(tailhook) make it parametrized
                if !path.exists() {
                    try_str!(mkdir_recursive(&path,
                        FilePermission::from_bits_truncate(0o755)));
                    try_str!(chown(&path, local.user_id as int, -1));
                }
                try!(bind_mount(&path, &dest));
                try_str!(mount_private(&dest));
            }
            Tmpfs(opt) => {
                try!(mount_tmpfs(&dest, opt.as_slice()));
            }
            Statedir(dir) => {
                let relative_dir = dir.path_relative_from(&root).unwrap();
                try!(bind_mount(
                    &state_dir.join(relative_dir),
                    &dest));
                try_str!(mount_private(&dest));
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

pub fn prepare_state_dir(dir: &Path, _global: &TreeConfig,
                         local: &ContainerConfig)
    -> Result<(), String>
{
    // TODO(tailhook) chown files
    if !dir.exists() {
        try_str!(mkdir(dir, FilePermission::from_bits_truncate(0o755)));
    }
    if local.resolv_conf.copy_from_host {
        try_str!(copy(&Path::new("/etc/resolv.conf"),
                      &dir.join("resolv.conf")));
    }
    if local.hosts_file.localhost || local.hosts_file.public_hostname {
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
        try_str!(chmod(&fname, FilePermission::from_bits_truncate(0o644)));
    }
    return Ok(());
}

pub fn read_local_config(global_cfg: &TreeConfig, child_cfg: &ChildConfig)
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
