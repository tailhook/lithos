extern crate rustc_serialize;
extern crate libc;
extern crate scan_dir;
extern crate argparse;
extern crate quire;
extern crate env_logger;
#[macro_use] extern crate log;
#[macro_use] extern crate lithos;


use std::env;
use std::fs::{metadata};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::default::Default;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue, Print};
use quire::parse_config;

use lithos::utils::{in_range, in_mapping, check_mapping, relative};
use lithos::master_config::MasterConfig;
use lithos::sandbox_config::SandboxConfig;
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

fn check_sandbox_config(sandbox: &SandboxConfig) {
    if sandbox.allow_users.len() == 0 {
        err!("No allowed users range. Please add `allow-users: [1-1000]`");
    }
    if sandbox.allow_groups.len() == 0 {
        err!("No allowed groups range. Please add `allow-groups: [1-1000]`");
    }
}

fn check(config_file: &Path, verbose: bool,
    sandbox_name: Option<String>, alter_config: Option<PathBuf>)
{
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

    let config_dir = config_file.parent().unwrap().join(&master.sandboxes_dir);
    scan_dir::ScanDir::files().read(&config_dir, |iter| {
        let yamls = iter.filter(|&(_, ref name)| name.ends_with(".yaml"));
        for (entry, current_fn) in yamls {
            // strip yaml suffix
            let current_name = &current_fn[..current_fn.len()-5];
            let sandbox: SandboxConfig = match parse_config(&entry.path(),
                &SandboxConfig::validator(), Default::default()) {
                Ok(cfg) => cfg,
                Err(e) => {
                    err!("Can't parse config: {}", e);
                    continue;
                }
            };
            check_sandbox_config(&sandbox);

            let default_config = config_file.parent().unwrap()
                .join(&master.processes_dir)
                .join(sandbox.config_file.as_ref().unwrap_or(
                    &PathBuf::from(&current_fn)));
            let config_file = match (current_name, &sandbox_name)
            {
                (name, &Some(ref t)) if name == t
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
                    &sandbox.image_dir
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
                    if !in_range(&sandbox.allow_users, config.user_id) {
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
                    if !in_range(&sandbox.allow_groups, config.group_id) {
                        err!("Group is not in allowed range (gid: {})",
                            config.group_id);
                    }
                }
                if !check_mapping(&sandbox.allow_users, &config.uid_map) {
                    err!("Bad uid mapping (probably doesn't match allow_users)");
                }
                if !check_mapping(&sandbox.allow_groups, &config.gid_map) {
                    err!("Bad gid mapping (probably doesn't match allow_groups)");
                }
            }
        }
    }).map_err(|e| {
        err!("Can't read config directory {:?}: {}", config_dir, e);
    }).ok();
    if alter_config.is_some() {
        err!("Tree {:?} is not used", sandbox_name);
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
    let mut sandbox_name = None;
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
             You must also specify --sandbox.")
          .metavar("DIR");
        ap.refer(&mut sandbox_name)
          .add_option(&["--sandbox", "--sandbox-name",
            // Compatibility names
            "-T", "--tree", "--subtree-name",
            ], ParseOption,
            "Name of the sandbox for which --config-dir takes effect")
          .metavar("NAME");
        ap.add_option(&["--version"],
            Print(env!("CARGO_PKG_VERSION").to_string()),
            "Show version");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                exit(x);
            }
        }
    }
    if alter_config.is_some() && sandbox_name.is_none() {
        err!("Please specify --sandbox if you use --dir");
    }
    check_binaries();
    check(&config_file, verbose, sandbox_name, alter_config);
    exit(EXIT_STATUS.load(Ordering::SeqCst) as i32);
}
