use std::io;
use std::io::{Write, BufWriter};
use std::fs::{File};
use std::fs::{create_dir_all, copy, metadata, symlink_metadata};
use std::path::{Path, PathBuf};
use std::collections::BTreeMap;

use libmount::{self, BindMount};
use failure::{Error, ResultExt, err_msg};

use lithos::mount::{mount_ro_recursive};
use lithos::mount::{mount_pseudo, mount_pts};
use lithos::network::{get_host_ip, get_host_name};
use lithos::master_config::MasterConfig;
use lithos::sandbox_config::SandboxConfig;
use lithos::container_config::{InstantiatedConfig, Volume};
use lithos::container_config::Volume::{Statedir, Readonly, Persistent, Tmpfs};
use lithos::utils::{set_file_mode, set_file_owner};
use lithos::utils::{relative};


fn map_dir(dir: &Path, dirs: &BTreeMap<PathBuf, PathBuf>) -> Option<PathBuf> {
    assert!(dir.is_absolute());
    for (prefix, real_dir) in dirs.iter() {
        if dir.starts_with(prefix) {
            return Some(real_dir.join(relative(dir, prefix)));
        }
    }
    return None;
}

fn prepare_resolv_conf(state_dir: &Path, local: &InstantiatedConfig,
    tree: &SandboxConfig)
    -> Result<(), Error>
{
    let path = state_dir.join("resolv.conf");
    if local.resolv_conf.copy_from_host {
        copy(&tree.resolv_conf, &path)?;
    }
    Ok(())
}

fn prepare_hosts_file(state_dir: &Path, local: &InstantiatedConfig,
    tree: &SandboxConfig)
    -> Result<(), Error>
{
    let copy_hosts = local.hosts_file.copy_from_host;
    let add_localhost = local.hosts_file.localhost.unwrap_or(!copy_hosts);
    let add_hostname = local.hosts_file.public_hostname.unwrap_or(!copy_hosts);
    if add_localhost || add_hostname || copy_hosts
        || tree.additional_hosts.len() > 0
    {
        let fname = state_dir.join("hosts");
        let mut file = BufWriter::new(
            File::create(&fname).context("cant create /state/hosts")?);
        if copy_hosts {
            let mut source = File::open(&tree.hosts_file)
                .map_err(|e| format_err!(
                    "error reading {:?}: {}", tree.hosts_file, e))?;
            io::copy(&mut source, &mut file)?;
            // In case file has no newline at the end and we are
            // going to add some records
            file.write_all(b"\n")?;
        }
        if add_localhost {
            file.write_all(
                "127.0.0.1 localhost.localdomain localhost\n"
                .as_bytes())?;
        }
        if add_hostname {
            writeln!(&mut file, "{} {}", get_host_ip()?, get_host_name()?)?;
        }
        for (ref host, ref ip) in tree.additional_hosts.iter() {
            writeln!(&mut file, "{} {}", ip, host)?;
        }
        set_file_mode(&fname, 0o644).ok(); // TODO(tailhook) check error?
    }
    Ok(())
}

pub fn prepare_state_dir(dir: &Path, local: &InstantiatedConfig,
    tree: &SandboxConfig)
    -> Result<(), String>
{
    _prepare_state_dir(dir, local, tree)
    .map_err(|e| format!("state dir: {}", e))
}

fn _prepare_state_dir(dir: &Path, local: &InstantiatedConfig,
    tree: &SandboxConfig)
    -> Result<(), Error>
{
    // TODO(tailhook) chown files
    if metadata(dir).is_err() {
        create_dir_all(dir)
            .map_err(|e| format_err!(
                "Couldn't create state directory: {}", e))?;
        set_file_mode(dir, 0o1777)
            .map_err(|e| format_err!(
                "Couldn't set chmod for state dir: {}", e))?;
    }

    prepare_resolv_conf(dir, local, tree)
        .map_err(|e| format_err!("error preparing resolf.conf: {}", e))?;
    prepare_hosts_file(dir, local, tree)
        .map_err(|e| format_err!("error preparing hosts: {}", e))?;
    return Ok(());
}

fn check_file(root: &Path, file: &str)
    -> Result<bool, Error>
{
    // TODO(tailhook) this is racy. And only fine because we expect image
    // not to change in while starting. This can't be enforced however, so
    // we need to use openat here
    let epath = root.join("etc");
    match symlink_metadata(&epath) {
        Ok(ref m) if m.is_dir() => {},
        Ok(_) => debug!("/etc is not a directory"),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => bail!("can't check /etc: {}", e),
    };
    let path = epath.join(file);
    match symlink_metadata(&path) {
        Ok(ref m) if m.is_file() => {},
        Ok(_) => debug!("/etc/{} is not a file", file),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => bail!("can't check /etc/{}: {}", file, e),
    };
    Ok(true)
}

fn mount_hosts_file(root: &Path, local: &InstantiatedConfig,
    state_dir: &Path)
    -> Result<(), Error>
{
    if local.hosts_file.mount == Some(false) {
        return Ok(());
    }
    match (check_file(root, "hosts")?, local.hosts_file.mount) {
        (false, None) => return Ok(()),
        (true, None) | (true, Some(true)) => {}
        (false, Some(true)) => {
            bail!("/etc/hosts is not a valid mount point");
        }
        (_, Some(false)) => return Ok(()),  // unreachable
    }
    BindMount::new(&state_dir.join("hosts"), &root.join("etc/hosts"))
    .mount().map_err(|e| format_err!("{}", e))
}

fn mount_resolv_conf(root: &Path, local: &InstantiatedConfig,
    state_dir: &Path)
    -> Result<(), Error>
{
    if local.resolv_conf.mount == Some(false) {
        return Ok(());
    }
    if local.resolv_conf.mount.is_none() && !local.resolv_conf.copy_from_host {
        return Ok(());
    }
    match (check_file(root, "resolv.conf")?, local.resolv_conf.mount) {
        (false, None) => return Ok(()),
        (true, None) | (true, Some(true)) => {}
        (false, Some(true)) => {
            bail!("/etc/resolf.conf is not a valid mount point");
        }
        (_, Some(false)) => return Ok(()),  // unreachable
    }
    BindMount::new(&state_dir.join("resolv.conf"),
                   &root.join("etc/resolv.conf"))
    .mount().map_err(|e| format_err!("{}", e))
}

pub fn setup_filesystem(master: &MasterConfig, tree: &SandboxConfig,
    local: &InstantiatedConfig, state_dir: &Path)
    -> Result<(), String>
{
    _setup_filesystem(master, tree, local, state_dir)
    .map_err(|e| format!("error setting up filesystem: {}", e))
}

fn _setup_filesystem(master: &MasterConfig, tree: &SandboxConfig,
    local: &InstantiatedConfig, state_dir: &Path)
    -> Result<(), Error>
{
    let root = PathBuf::from("/");
    let mntdir = master.runtime_dir.join(&master.mount_dir);
    assert!(mntdir.is_absolute());

    let mut volumes: Vec<(&String, &Volume)> = local.volumes.iter().collect();
    volumes.sort_by(|&(mp1, _), &(mp2, _)| mp1.len().cmp(&mp2.len()));

    let devdir = mntdir.join("dev");
    BindMount::new(&master.devfs_dir, &devdir).mount()
        .map_err(|e| format_err!("{}", e))?;
    mount_ro_recursive(&devdir).map_err(err_msg)?;

    mount_pts(&mntdir.join("dev/pts")).map_err(err_msg)?;
    mount_pseudo(&mntdir.join("sys"), "sysfs", "", true).map_err(err_msg)?;
    mount_pseudo(&mntdir.join("proc"), "proc", "", false).map_err(err_msg)?;

    for &(mp_str, volume) in volumes.iter() {
        let tmp_mp = PathBuf::from(&mp_str[..]);
        assert!(tmp_mp.is_absolute());  // should be checked earlier

        let dest = mntdir.join(relative(&tmp_mp, &root));
        match volume {
            &Readonly(ref dir) => {
                let path = match map_dir(dir, &tree.readonly_paths).or_else(
                                 || map_dir(dir, &tree.writable_paths)) {
                    None => {
                        bail!(concat!("Can't find volume for {},",
                            " probably missing entry in readonly-paths"),
                            dir.display());
                    }
                    Some(path) => path,
                };
                BindMount::new(&path, &dest).mount()
                    .map_err(|e| format_err!("{}", e))?;
                mount_ro_recursive(&dest).map_err(err_msg)?;
            }
            &Persistent(ref opt) => {
                let path = match map_dir(&opt.path, &tree.writable_paths) {
                    None => {
                        bail!("Can't find volume for {:?}, \
                            probably missing entry in writable-paths",
                            opt.path);
                    }
                    Some(path) => path,
                };
                if metadata(&path).is_err() {
                    if opt.mkdir {
                        create_dir_all(&path)
                            .map_err(|e| format_err!("Error creating \
                                persistent volume: {}", e))?;
                        let user = local.map_uid(opt.user)
                            .ok_or(format_err!(
                                "Non-mapped user {} for volume {}",
                                opt.user, mp_str))?;
                        let group = local.map_gid(opt.group)
                            .ok_or(format_err!(
                                "Non-mapped group {} for volume {}",
                                opt.group, mp_str))?;
                        set_file_owner(&path, user, group)
                            .map_err(|e| format_err!("Error chowning \
                                persistent volume: {}", e))?;
                        set_file_mode(&path, opt.mode)
                            .map_err(|e| format_err!("Can't chmod persistent \
                                volume: {}", e))?;
                    }
                }
                BindMount::new(&path, &dest).mount()
                    .map_err(|e| format_err!("{}", e))?;
            }
            &Tmpfs(ref opt) => {
                libmount::Tmpfs::new(&dest)
                    .size_bytes(opt.size).mode(opt.mode)
                    .mount()
                    .map_err(|e| format_err!("{}", e))?;
            }
            &Statedir(ref opt) => {
                let relative_dir = relative(&opt.path, &root);
                let dir = state_dir.join(&relative_dir);
                if Path::new(&relative_dir) != Path::new(".") {
                    create_dir_all(&dir)
                        .map_err(|e| format_err!("Error creating \
                            persistent volume: {}", e))?;
                    let user = local.map_uid(opt.user)
                        .ok_or(format_err!("Non-mapped user {} for volume {}",
                            opt.user, mp_str))?;
                    let group = local.map_gid(opt.group)
                        .ok_or(format_err!("Non-mapped group {} for volume {}",
                            opt.group, mp_str))?;
                    set_file_owner(&dir, user, group)
                        .map_err(|e| format_err!("Error chowning \
                            persistent volume: {}", e))?;
                    set_file_mode(&dir, opt.mode)
                        .map_err(|e| format_err!("Can't chmod persistent \
                            volume: {}", e))?;
                }
                BindMount::new(&dir, &dest).mount()
                    .map_err(|e| format_err!("{}", e))?;
            }
        }
    }

    mount_resolv_conf(&mntdir, local, state_dir)?;
    mount_hosts_file(&mntdir, local, state_dir)?;

    return Ok(());
}
