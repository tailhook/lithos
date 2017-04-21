extern crate argparse;
extern crate env_logger;
extern crate libc;
extern crate lithos;
extern crate quire;
extern crate rustc_serialize;
extern crate scan_dir;
#[macro_use] extern crate log;


use std::env;
use std::fs::{metadata};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue, Print, Collect};
use quire::{parse_config, Options};

use lithos::utils::{in_range, in_mapping, check_mapping, relative};
use lithos::master_config::MasterConfig;
use lithos::sandbox_config::SandboxConfig;
use lithos::container_config::{ContainerConfig, Variables};
use lithos::child_config::ChildConfig;
use lithos::network::{get_host_name, get_host_ip};
use lithos::id_map::{IdMapExt};

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
    // TODO(tailhook) check allow_users/allow_groups against uid_map/gid_map
}

fn check_container(config_file: &Path) -> Result<ContainerConfig, ()>
{
    // Only checks things that can be checked without other configs
    let config: ContainerConfig = match parse_config(config_file,
        &ContainerConfig::validator(), &Options::default())
    {
        Ok(cfg) => cfg,
        Err(e) => {
            err!("Can't read container config {:?}: {}", config_file, e);
            return Err(());
        }
    };
    if config.uid_map.len() > 0 {
        if !in_mapping(&config.uid_map, config.user_id) {
            err!("User is not in mapped range (uid: {})",
                config.user_id);
        }
    }
    if config.gid_map.len() > 0 {
        if !in_mapping(&config.gid_map, config.group_id) {
            err!("Group is not in mapped range (gid: {})",
                config.user_id);
        }
    }
    Ok(config)
}

fn check(config_file: &Path, verbose: bool,
    sandbox_name: Option<String>, alter_config: Option<PathBuf>)
{
    let mut alter_config = alter_config;
    let master: MasterConfig = match parse_config(&config_file,
        &MasterConfig::validator(), &Options::default()) {
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
                &SandboxConfig::validator(), &Options::default()) {
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
                &ChildConfig::mapping_validator(), &Options::default()) {
                Ok(cfg) => cfg,
                Err(e) => {
                    warn!("Can't read child config {:?}: {}", config_file, e);
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
                let config = match check_container(&sandbox.image_dir
                    .join(&child_cfg.image)
                    .join(&relative(cfg_path, &Path::new("/"))))
                {
                    Ok(config) => config,
                    Err(()) => continue,
                };
                // Uidmaps aren't substituted
                if config.uid_map.len() > 0 {
                    if sandbox.uid_map.len() > 0 {
                        err!("Can't have uid_maps in both the sandbox and a \
                              container itself");
                    }
                } else {
                    if sandbox.uid_map.len() > 0 {
                        if sandbox.uid_map.map_id(config.user_id).is_none() {
                            err!("User is not in mapped range \
                                (uid: {})",
                                config.user_id);
                        }
                    }
                    if !in_range(&sandbox.allow_users, config.user_id) {
                        err!("User is not in allowed range (uid: {})",
                            config.user_id);
                    }
                }
                if config.gid_map.len() > 0 {
                    if sandbox.gid_map.len() > 0 {
                        err!("Can't have uid_maps in both the sandbox and a \
                              container itself");
                    }
                } else {
                    if sandbox.gid_map.len() > 0 {
                        if sandbox.gid_map.map_id(config.group_id).is_none() {
                            err!("Group is not in mapped range \
                                (gid: {})",
                                config.group_id);
                        }
                    }
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
                // Per-instance validation
                for (key, typ) in &config.variables {
                    if let Some(value) = child_cfg.variables.get(key) {
                        if let Err(e) = typ.validate(value, &sandbox) {
                            err!("Variable {:?} is invalid: {}", key, e);
                        }
                    } else {
                        err!("Variable {:?} is undefined", key);
                    }
                }
                for i in 0..child_cfg.instances {
                    let name = format!("{}/{}.{}",
                        sandbox_name.as_ref().map(|x| &x[..])
                            .unwrap_or("unknown-sandbox"),
                        child_name, i);
                    let icfg = match config.instantiate(&Variables {
                            user_vars: &child_cfg.variables,
                            lithos_name: &name,
                            lithos_config_filename: &child_cfg.config,
                        }) {
                        Ok(x) => x,
                        Err(e) => {
                            err!("Variable substitution error {:?} \
                                of sandbox {:?} of image {:?}: {}",
                                &child_cfg.config, sandbox_name,
                                child_cfg.image,
                                e.join("; "));
                            continue;
                        }
                    };
                    // TODO(tailhook) check tcp ports
                    for (port, _) in icfg.tcp_ports {
                        if !in_range(&sandbox.allow_tcp_ports, port as u32) {
                            err!("Port {} is not allowed for {:?} \
                                of sandbox {:?} of image {:?}",
                                port, &child_cfg.config, sandbox_name,
                                child_cfg.image);
                        }
                    }
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
    let mut check_containers = Vec::<String>::new();
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
          .metavar("FILE");
        ap.refer(&mut sandbox_name)
          .add_option(&["--sandbox", "--sandbox-name",
            // Compatibility names
            "-T", "--tree", "--subtree-name",
            ], ParseOption,
            "Name of the sandbox for which --config-dir takes effect")
          .metavar("NAME");
        ap.refer(&mut check_containers)
          .add_option(&["--check-container"], Collect, "
            Instead of checking full lithos configuration check the
            container's configuration in the FILE. This is useful to check
            container locally, where you don't have lithos configured,
            before actually uploading the container. Multiple files may be
            specified in multiple arguments.
            ")
          .metavar("FILE");
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
    if check_containers.len() > 0 {
        for file in &check_containers {
            check_container(Path::new(file)).ok();
        }
    } else {
        check_binaries();
        check(&config_file, verbose, sandbox_name, alter_config);
    }
    exit(EXIT_STATUS.load(Ordering::SeqCst) as i32);
}
