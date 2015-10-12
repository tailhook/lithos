extern crate rustc_serialize;
extern crate libc;
extern crate regex;

extern crate argparse;
extern crate quire;
extern crate env_logger;
#[macro_use] extern crate log;
#[macro_use] extern crate lithos;


use std::env;
use std::fs::{read_dir, metadata};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::default::Default;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

use regex::Regex;
use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue};
use quire::parse_config;

use lithos::utils::{in_range, in_mapping, check_mapping, relative};
use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;
use lithos::container_config::ContainerConfig;
use lithos::child_config::ChildConfig;
use lithos::network::{get_host_name, get_host_ip};

static EXIT_STATUS: AtomicUsize = ATOMIC_USIZE_INIT;

macro_rules! err {
    ( $( $x:expr ),* ) => {
        {
            error!($($x),*);
            EXIT_STATUS.store(1,  Ordering::SeqCst);
        }
    }
}


fn check_master_config(master: &MasterConfig, verbose: bool) {
    // TODO(tailhook) maybe check host only if we need it for hosts file
    match get_host_name() {
        Ok(hostname) => {
            if verbose {
                println!("Hostname is {}", hostname);
            }
        }
        Err(e) => {
            err!("Can't get hostname: {}", e);
        }
    }
    match get_host_ip() {
        Ok(ipaddr) => {
            if verbose {
                println!("IPAddr is {}", ipaddr);
            }
        }
        Err(e) => {
            err!("Can't get IPAddress: {}", e);
        }
    }

    if metadata(&master.devfs_dir).is_err() {
        err!("Devfs dir ({:?}) must exist and contain device nodes",
            master.devfs_dir);
    }
}

fn check_tree_config(tree: &TreeConfig) {
    if tree.allow_users.len() == 0 {
        err!("No allowed users range. Please add `allow-users: [1-1000]`");
    }
    if tree.allow_groups.len() == 0 {
        err!("No allowed groups range. Please add `allow-groups: [1-1000]`");
    }
}

fn check(config_file: &Path, verbose: bool,
    tree_name: Option<String>, alter_config: Option<PathBuf>)
{
    let name_re = Regex::new(r"^([\w-]+)\.yaml$").unwrap();
    let mut alter_config = alter_config;
    let master: MasterConfig = match parse_config(&config_file,
        &MasterConfig::validator(), Default::default()) {
        Ok(cfg) => cfg,
        Err(e) => {
            err!("Can't parse config: {}", e);
            return;
        }
    };

    check_master_config(&master, verbose);

    let config_dir = config_file.parent().unwrap().join(master.sandboxes_dir);
    for tree_fn in read_dir(&config_dir)
        .map(|v| v.collect())
        .map_err(|e| {
            err!("Can't open config directory {:?}: {}", config_dir, e);
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
                err!("Can't parse config: {}", e);
                continue;
            }
        };
        check_tree_config(&tree);

        let default_config = config_file.parent().unwrap()
            .join(&master.processes_dir)
            .join(tree.config_file.as_ref().unwrap_or(
                &PathBuf::from(tree_fn.file_name())));
        let config_file = match (
            tree_fn.path().file_stem().and_then(|x| x.to_str()), &tree_name)
        {
            (Some(ref f), &Some(ref t)) if &f[..] == t
            => alter_config.take().unwrap_or(default_config),
            _ => default_config,
        };

        debug!("Checking {:?}", config_file);
        let all_children: BTreeMap<String, ChildConfig>;
        all_children = match parse_config(&config_file,
            &ChildConfig::mapping_validator(), Default::default()) {
            Ok(cfg) => cfg,
            Err(e) => {
                err!("Can't read child config {:?}: {}", config_file, e);
                continue;
            }
        };
        for (ref child_name, ref child_cfg) in all_children.iter() {
            let cfg_path = Path::new(&child_cfg.config);
            if !cfg_path.is_absolute() {
                err!("Config path must be absolute");
                continue;
            }
            debug!("Opening config for {:?}", child_name);
            let config: ContainerConfig = match parse_config(
                &tree.image_dir
                    .join(&child_cfg.image)
                    .join(&relative(cfg_path, &Path::new("/"))),
                &ContainerConfig::validator(), Default::default()) {
                Ok(cfg) => cfg,
                Err(e) => {
                    err!("Can't read child config {:?}: {}.\n\
                        Sometimes the reason of reading configuration is \
                        absolute symlinks for config, in that case it may \
                        work in real daemon, but better fix it.",
                        child_name, e);
                    continue;
                }
            };
            if config.uid_map.len() > 0 {
                if !in_mapping(&config.uid_map, config.user_id) {
                    err!("User is not in mapped range (uid: {})",
                        config.user_id);
                }
            } else {
                if !in_range(&tree.allow_users, config.user_id) {
                    err!("User is not in allowed range (uid: {})",
                        config.user_id);
                }
            }
            if config.gid_map.len() > 0 {
                if !in_mapping(&config.gid_map, config.group_id) {
                    err!("Group is not in mapped range (gid: {})",
                        config.user_id);
                }
            } else {
                if !in_range(&tree.allow_groups, config.group_id) {
                    err!("Group is not in allowed range (gid: {})",
                        config.group_id);
                }
            }
            if !check_mapping(&tree.allow_users, &config.uid_map) {
                err!("Bad uid mapping (probably doesn't match allow_users)");
            }
            if !check_mapping(&tree.allow_groups, &config.gid_map) {
                err!("Bad gid mapping (probably doesn't match allow_groups)");
            }
        }
    }
    if alter_config.is_some() {
        err!("Tree {:?} is not used", tree_name);
    }
}

fn check_binaries() {
    let dir = match env::current_exe().ok()
        .and_then(|x| x.parent().map(|x| x.to_path_buf()))
    {
        Some(dir) => dir,
        None => {
            err!("Can't find out exe path");
            return;
        }
    };
    if metadata(&dir.join("lithos_tree")).is_err() {
        err!("Can't find lithos_tree binary");
    }
    if metadata(&dir.join("lithos_knot")).is_err() {
        err!("Can't find lithos_knot binary");
    }
}

fn main() {

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let mut config_file = PathBuf::from("/etc/lithos/master.yaml");
    let mut verbose = false;
    let mut alter_config = None;
    let mut tree_name = None;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Checks if lithos configuration is ok");
        ap.refer(&mut config_file)
          .add_option(&["-C", "--config"], Parse,
            "Name of the global configuration file \
             (default /etc/lithos/master.yaml)")
          .metavar("FILE");
        ap.refer(&mut verbose)
          .add_option(&["-v", "--verbose"], StoreTrue,
            "Verbose output");
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
        err!("Please specify --tree if you use --dir");
    }
    check_binaries();
    check(&config_file, verbose, tree_name, alter_config);
    exit(EXIT_STATUS.load(Ordering::SeqCst) as i32);
}
