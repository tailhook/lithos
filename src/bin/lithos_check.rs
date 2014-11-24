#![feature(phase, macro_rules, if_let)]

extern crate serialize;
extern crate libc;
#[phase(plugin, link)] extern crate log;
extern crate regex;
#[phase(plugin)] extern crate regex_macros;
extern crate time;
extern crate debug;

extern crate argparse;
extern crate quire;
#[phase(plugin, link)] extern crate lithos;


use std::os::{getenv, setenv};
use std::os::args;
use std::io::fs::readdir;
use std::os::{set_exit_status, self_exe_path};
use std::io::fs::PathExtensions;
use std::default::Default;

use argparse::{ArgumentParser, Store, StoreOption, StoreTrue};
use quire::parse_config;

use lithos::signal;
use lithos::utils::{in_range, in_mapping, check_mapping};
use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;
use lithos::child_config::ChildConfig;
use lithos::network::{get_host_name, get_host_ip};


fn check_master_config(master: &MasterConfig, verbose: bool) {
    // TODO(tailhook) maybe check host only if we need it for hosts file
    match get_host_name() {
        Ok(hostname) => {
            if verbose {
                println!("Hostname is {}", hostname);
            }
        }
        Err(e) => {
            warn!("Can't get hostname: {}", e);
            set_exit_status(1);
        }
    }
    match get_host_ip() {
        Ok(ipaddr) => {
            if verbose {
                println!("IPAddr is {}", ipaddr);
            }
        }
        Err(e) => {
            warn!("Can't get IPAddress: {}", e);
            set_exit_status(1);
        }
    }

    if !master.devfs_dir.exists() {
        error!("Devfs dir ({}) must exist and contain device nodes",
            master.devfs_dir.display());
        set_exit_status(1);
    }
}

fn check_tree_config(tree: &TreeConfig) {
    if tree.allow_users.len() == 0 {
        error!("No allowed users range. Please add `allow-users: [1-1000]`");
        set_exit_status(1);
    }
    if tree.allow_groups.len() == 0 {
        error!("No allowed groups range. Please add `allow-groups: [1-1000]`");
        set_exit_status(1);
    }
}

fn check(config_file: Path, verbose: bool,
    tree_name: Option<String>, replacement_dir: Option<Path>)
{
    let name_re = regex!(r"^([\w-]+)\.yaml$");
    let mut replacement_dir = replacement_dir;
    let master: MasterConfig = match parse_config(&config_file,
        &*MasterConfig::validator(), Default::default()) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Can't parse config: {}", e);
            set_exit_status(1);
            return;
        }
    };

    check_master_config(&master, verbose);

    for tree_fn in readdir(&master.config_dir)
        .map_err(|e| {
            error!("Can't open config directory {}: {}",
                master.config_dir.display(), e);
            set_exit_status(1);
        })
        .unwrap_or(Vec::new())
        .into_iter()
        .filter(|f| f.filename_str()
                     .map(|s| name_re.is_match(s))
                     .unwrap_or(false))
    {
        let tree: TreeConfig = match parse_config(&tree_fn,
            &*TreeConfig::validator(), Default::default()) {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("Can't parse config: {}", e);
                set_exit_status(1);
                continue;
            }
        };
        check_tree_config(&tree);

        let config_dir = match (tree_fn.filestem_str(), &tree_name) {
            (Some(ref f), &Some(ref t)) if *f == t.as_slice()
            => replacement_dir.take().unwrap_or(tree.config_dir),
            _ => tree.config_dir,
        };

        debug!("Checking child dir {}", config_dir.display());
        let dirlist = match readdir(&config_dir) {
            Ok(dirlist) => dirlist,
            Err(e) => {
                error!("Can't open config directory {}: {}",
                    config_dir.display(), e);
                set_exit_status(1);
                return;
            }
        };
        for child_fn in dirlist.into_iter() {
            match (child_fn.filestem_str(), child_fn.extension_str()) {
                (Some(""), _) => continue,  // Hidden files
                (_, Some("yaml")) => {}
                _ => continue,  // Non-yaml, old, whatever, files
            }
            debug!("Checking {}", child_fn.display());
            let child_cfg: ChildConfig = match parse_config(&child_fn,
                &*ChildConfig::validator(), Default::default()) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!("Can't read child config {}: {}",
                        child_fn.display(), e);
                    set_exit_status(1);
                    continue;
                }
            };
            let cfg_path = Path::new(child_cfg.config);
            if !cfg_path.is_absolute() {
                error!("Config path must be absolute");
                set_exit_status(1);
                continue;
            }
            debug!("Opening config {}", child_fn.display());
            let config: ContainerConfig = match parse_config(
                &tree.image_dir
                    .join(child_cfg.image)
                    .join(cfg_path.path_relative_from(
                        &Path::new("/")).unwrap()),
                &*ContainerConfig::validator(), Default::default()) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!(concat!("Can't read child config {}: {}.",
                        "Sometimes the reason is absolute symlinks for config, ",
                        "in that case it may work in real daemon, but better ",
                        "fix it."), child_fn.display(), e);
                    set_exit_status(1);
                    continue;
                }
            };
            if config.uid_map.len() > 0 {
                if !in_mapping(&config.uid_map, config.user_id) {
                    error!("User is not in mapped range (uid: {})",
                        config.user_id);
                    set_exit_status(1);
                }
            } else {
                if !in_range(&tree.allow_users, config.user_id) {
                    error!("User is not in allowed range (uid: {})",
                        config.user_id);
                    set_exit_status(1);
                }
            }
            if config.gid_map.len() > 0 {
                if !in_mapping(&config.gid_map, config.group_id) {
                    error!("Group is not in mapped range (gid: {})",
                        config.user_id);
                    set_exit_status(1);
                }
            } else {
                if !in_range(&tree.allow_groups, config.group_id) {
                    error!("Group is not in allowed range (gid: {})",
                        config.group_id);
                    set_exit_status(1);
                }
            }
            if !check_mapping(&tree.allow_users, &config.uid_map) {
                error!("Bad uid mapping (probably doesn't match allow_users)");
                set_exit_status(1);
            }
            if !check_mapping(&tree.allow_groups, &config.gid_map) {
                error!("Bad gid mapping (probably doesn't match allow_groups)");
                set_exit_status(1);
            }
        }
    }
    if replacement_dir.is_some() {
        error!("Tree {} is not used", tree_name);
        set_exit_status(1);
    }
}

fn check_binaries() {
    let dir = match self_exe_path() {
        Some(dir) => dir,
        None => {
            error!("Can't find out exe path");
            set_exit_status(1);
            return;
        }
    };
    if !dir.join("lithos_tree").exists() {
        error!("Can't find lithos_tree binary");
        set_exit_status(1);
    }
    if !dir.join("lithos_knot").exists() {
        error!("Can't find lithos_knot binary");
        set_exit_status(1);
    }
}

fn main() {

    signal::block_all();
    if getenv("RUST_LOG").is_none() {
        setenv("RUST_LOG", "warn");
    }

    let mut config_file = Path::new("/etc/lithos.yaml");
    let mut verbose = false;
    let mut config_dir = None;
    let mut tree_name = None;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Checks if lithos configuration is ok");
        ap.refer(&mut config_file)
          .add_option(["-C", "--config"], box Store::<Path>,
            "Name of the global configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut verbose)
          .add_option(["-v", "--verbose"], box StoreTrue,
            "Verbose configuration");
        ap.refer(&mut config_dir)
          .add_option(["-D", "--dir", "--config-dir"], box StoreOption::<Path>,
            concat!("Name of the alterate directory with configs. ",
                    "Useful to test configuration directory before ",
                    "switching it to be primary one. ",
                    "You must also specify --tree."))
          .metavar("DIR");
        ap.refer(&mut tree_name)
          .add_option(["-T", "--tree", "--subtree-name"],
            box StoreOption::<String>,
            concat!("Name of the tree for which --config-dir takes effect"))
          .metavar("NAME");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    if config_dir.is_some() && tree_name.is_none() {
        error!("Please specify --tree if you use --dir");
        set_exit_status(1);
        return;
    }
    check_binaries();
    check(config_file, verbose, tree_name, config_dir);
}
