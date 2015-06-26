extern crate rustc_serialize;
extern crate libc;
#[macro_use] extern crate log;
extern crate regex;

extern crate argparse;
extern crate quire;
#[macro_use] extern crate lithos;


use std::env;
use std::fs::{read_dir, metadata};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::default::Default;
use std::collections::BTreeMap;

use regex::Regex;
use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue};
use quire::parse_config;

use lithos::utils::{in_range, in_mapping, check_mapping, relative};
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
            exit(1); // TODO(tailhook) set exit status but don't exit
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
            exit(1); // TODO(tailhook) set exit status but don't exit
        }
    }

    if metadata(&master.devfs_dir).is_err() {
        error!("Devfs dir ({:?}) must exist and contain device nodes",
            master.devfs_dir);
        exit(1); // TODO(tailhook) set exit status but don't exit
    }
}

fn check_tree_config(tree: &TreeConfig) {
    if tree.allow_users.len() == 0 {
        error!("No allowed users range. Please add `allow-users: [1-1000]`");
        exit(1); // TODO(tailhook) set exit status but don't exit
    }
    if tree.allow_groups.len() == 0 {
        error!("No allowed groups range. Please add `allow-groups: [1-1000]`");
        exit(1); // TODO(tailhook) set exit status but don't exit
    }
}

fn check(config_file: &Path, verbose: bool,
    tree_name: Option<String>, alter_config: Option<PathBuf>)
{
    let name_re = Regex::new(r"^([\w-]+)\.yaml$").unwrap();
    let mut alter_config = alter_config;
    let master: MasterConfig = match parse_config(&config_file,
        &*MasterConfig::validator(), Default::default()) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Can't parse config: {}", e);
            exit(1); // TODO(tailhook) set exit status but don't exit
            //return;
        }
    };

    check_master_config(&master, verbose);

    for tree_fn in read_dir(&master.config_dir)
        .map(|v| v.collect())
        .map_err(|e| {
            error!("Can't open config directory {:?}: {}",
                master.config_dir, e);
            exit(1);
        })
        .unwrap_or(Vec::new())
        .into_iter()
        .filter_map(|f| f.ok())
        .filter(|f| f.file_name().into_string().ok()
                     .map(|fname| name_re.is_match(&fname))
                     .unwrap_or(false))
    {
        let tree: TreeConfig = match parse_config(&tree_fn.path(),
            &*TreeConfig::validator(), Default::default()) {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("Can't parse config: {}", e);
                exit(1); // TODO(tailhook) set exit status but don't exit
                //continue;
            }
        };
        check_tree_config(&tree);

        let config_file = match (
            tree_fn.path().file_stem().and_then(|x| x.to_str()), &tree_name)
        {
            (Some(ref f), &Some(ref t)) if &f[..] == t
            => alter_config.take().unwrap_or(tree.config_file),
            _ => tree.config_file,
        };

        debug!("Checking {:?}", config_file);
        let all_children: BTreeMap<String, ChildConfig>;
        all_children = match parse_config(&config_file,
            &*ChildConfig::mapping_validator(), Default::default()) {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("Can't read child config {:?}: {}", config_file, e);
                exit(1); // TODO(tailhook) set exit status but don't exit
                //continue;
            }
        };
        for (ref child_name, ref child_cfg) in all_children.iter() {
            let cfg_path = Path::new(&child_cfg.config);
            if !cfg_path.is_absolute() {
                error!("Config path must be absolute");
                exit(1); // TODO(tailhook) set exit status but don't exit
                //continue;
            }
            debug!("Opening config for {:?}", child_name);
            let config: ContainerConfig = match parse_config(
                &tree.image_dir
                    .join(&child_cfg.image)
                    .join(&relative(cfg_path, &Path::new("/"))),
                &*ContainerConfig::validator(), Default::default()) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!("Can't read child config {:?}: {}.\n\
                        Sometimes the reason of reading configuration is \
                        absolute symlinks for config, in that case it may \
                        work in real daemon, but better fix it.",
                        child_name, e);
                    exit(1); // TODO(tailhook) set exit status but don't exit
                    //continue;
                }
            };
            if config.uid_map.len() > 0 {
                if !in_mapping(&config.uid_map, config.user_id) {
                    error!("User is not in mapped range (uid: {})",
                        config.user_id);
                    exit(1); // TODO(tailhook) set exit status but don't exit
                }
            } else {
                if !in_range(&tree.allow_users, config.user_id) {
                    error!("User is not in allowed range (uid: {})",
                        config.user_id);
                    exit(1); // TODO(tailhook) set exit status but don't exit
                }
            }
            if config.gid_map.len() > 0 {
                if !in_mapping(&config.gid_map, config.group_id) {
                    error!("Group is not in mapped range (gid: {})",
                        config.user_id);
                    exit(1); // TODO(tailhook) set exit status but don't exit
                }
            } else {
                if !in_range(&tree.allow_groups, config.group_id) {
                    error!("Group is not in allowed range (gid: {})",
                        config.group_id);
                    exit(1); // TODO(tailhook) set exit status but don't exit
                }
            }
            if !check_mapping(&tree.allow_users, &config.uid_map) {
                error!("Bad uid mapping (probably doesn't match allow_users)");
                exit(1); // TODO(tailhook) set exit status but don't exit
            }
            if !check_mapping(&tree.allow_groups, &config.gid_map) {
                error!("Bad gid mapping (probably doesn't match allow_groups)");
                exit(1); // TODO(tailhook) set exit status but don't exit
            }
        }
    }
    if alter_config.is_some() {
        error!("Tree {:?} is not used", tree_name);
        exit(1); // TODO(tailhook) set exit status but don't exit
    }
}

fn check_binaries() {
    let dir = match env::current_exe().ok()
        .and_then(|x| x.parent().map(|x| x.to_path_buf()))
    {
        Some(dir) => dir,
        None => {
            error!("Can't find out exe path");
            exit(1); // TODO(tailhook) set exit status but don't exit
            //return;
        }
    };
    if metadata(&dir.join("lithos_tree")).is_err() {
        error!("Can't find lithos_tree binary");
        exit(1); // TODO(tailhook) set exit status but don't exit
    }
    if metadata(&dir.join("lithos_knot")).is_err() {
        error!("Can't find lithos_knot binary");
        exit(1); // TODO(tailhook) set exit status but don't exit
    }
}

fn main() {

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }

    let mut config_file = PathBuf::from("/etc/lithos.yaml");
    let mut verbose = false;
    let mut alter_config = None;
    let mut tree_name = None;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Checks if lithos configuration is ok");
        ap.refer(&mut config_file)
          .add_option(&["-C", "--config"], Parse,
            "Name of the global configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut verbose)
          .add_option(&["-v", "--verbose"], StoreTrue,
            "Verbose configuration");
        ap.refer(&mut alter_config)
          .add_option(&["--alternate-config"], ParseOption,
            "Name of the alterate file name with configs.
             Useful to test configuration file before
             switching it to be primary one.
             You must also specify --tree.")
          .metavar("DIR");
        ap.refer(&mut tree_name)
          .add_option(&["-T", "--tree", "--subtree-name"], ParseOption,
            "Name of the tree for which --config-dir takes effect")
          .metavar("NAME");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                exit(x);
            }
        }
    }
    if alter_config.is_some() && tree_name.is_none() {
        error!("Please specify --tree if you use --dir");
        exit(1);
    }
    check_binaries();
    check(&config_file, verbose, tree_name, alter_config);
}
