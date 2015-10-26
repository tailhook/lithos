#[macro_use] extern crate log;
extern crate env_logger;
extern crate argparse;
extern crate quire;
extern crate lithos;
extern crate time;
extern crate scan_dir;
extern crate rustc_serialize;

use std::env;
use std::io::{BufReader, BufRead};
use std::fs::{File, remove_dir_all};
use std::path::{PathBuf, Path};
use std::process::exit;
use std::collections::HashSet;
use std::collections::BTreeMap;
use std::collections::VecDeque;

use time::{Tm, Duration, now_utc};
use quire::parse_config;
use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue, StoreConst};
use argparse::{Print};
use rustc_serialize::json;

use lithos::master_config::MasterConfig;
use lithos::tree_config::TreeConfig;
use lithos::child_config::ChildConfig;


#[derive(Clone, Copy, Debug)]
enum Action {
    Used,
    Unused,
    DeleteUnused,
}


fn main() {

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();
    let mut config_file = PathBuf::from("/etc/lithos/master.yaml");
    let mut verbose = false;
    let mut ver_min = 0;
    let mut ver_max = 1000;
    let mut action = Action::Used;
    let mut days = None::<u32>;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Show used/unused images and clean if needed");
        ap.refer(&mut config_file)
          .add_option(&["-C", "--config"], Parse,
            "Name of the global configuration file \
             (default /etc/lithos/master.yaml)")
          .metavar("FILE");
        ap.refer(&mut days)
          .add_option(&["-D", "--history-days"], ParseOption,
            r"Keep images that used no more than DAYS ago.
              There is no reasonable default, so you should specify this
              argument or --versions-min to get sane behavior.")
          .metavar("DAYS");
        ap.refer(&mut ver_min)
          .add_option(&["--vmin", "--versions-min"], Parse,
            r"Keep minimum NUM versions (even if they are older than DAYS).
              Default is 0 which means keep only current version.")
          .metavar("NUM");
        ap.refer(&mut ver_max)
          .add_option(&["--vmax", "--versions-max"], Parse,
            r"Keep maximum NUM versions
              (even if need to delete images more recent than DAYS).
              Default is 1000.")
          .metavar("NUM");
        ap.refer(&mut verbose)
          .add_option(&["-v", "--verbose"], StoreTrue,
            "Verbose output");
        ap.refer(&mut action)
          .add_option(&["--used"], StoreConst(Action::Used),
            "Show used images")
          .add_option(&["--unused"], StoreConst(Action::Unused),
            "Show unused images")
          .add_option(&["--delete-unused"], StoreConst(Action::DeleteUnused),
            "Delete unused images");
        ap.add_option(&["--version"],
            Print(env!("CARGO_PKG_VERSION").to_string()),
            "Show version of the lithos");
        ap.parse_args_or_exit();
    }
    let master: MasterConfig = match parse_config(&config_file,
        &MasterConfig::validator(), Default::default()) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Can't parse config: {}", e);
            exit(1);
        }
    };
    let tm = days.map(|days| now_utc() - Duration::days(days as i64));
    let (used, dirs) = match find_used_images(&master, &config_file,
        tm, ver_min, ver_max)
    {
        Ok((used, dirs)) => (used, dirs),
        Err(e) => {
            error!("Error finding out used images: {}", e);
            exit(1);
        }
    };
    match action {
        Action::Used => {
            for i in used {
                println!("{:?}", i);
            }
        }
        Action::Unused => {
            let unused = find_unused(&used, &dirs)
                .map_err(|e| {
                    error!("Error finding unused images: {:?}", e);
                    exit(2);
                })
                .unwrap();
            for i in unused {
                println!("{:?}", i);
            }
        }
        Action::DeleteUnused => {
            let unused = find_unused(&used, &dirs)
                .map_err(|e| {
                    error!("Error finding unused images: {:?}", e);
                    exit(2);
                })
                .unwrap();
            for i in unused {
                if verbose {
                    println!("Deleting {:?}", i);
                }
                remove_dir_all(&i)
                    .map_err(|e| error!("Error removing {:?}: {}", i, e)).ok();
            }
        }
    }
}

fn find_unused(used: &HashSet<PathBuf>, dirs: &HashSet<PathBuf>)
    -> Result<Vec<PathBuf>, scan_dir::Error>
{
    let mut unused = Vec::new();
    for dir in dirs.iter() {
        try!(scan_dir::ScanDir::dirs().read(dir, |iter| {
            for (entry, _) in iter {
                // we know that file_type works, because scan_dir already
                // tried
                let typ = entry.file_type().unwrap();

                if typ.is_symlink() {
                    // Temporary fix: symlinks are ignored
                    // TODO(tailhook) symlinks are used images (optionally)
                    continue;
                }
                let path = entry.path().to_path_buf();
                if !used.contains(&path) {
                    unused.push(path);
                }
            }
        }));
    }
    Ok(unused)
}

fn find_used_images(master: &MasterConfig, master_file: &Path,
    min_time: Option<Tm>, ver_min: u32, ver_max: u32)
    -> Result<(HashSet<PathBuf>, HashSet<PathBuf>), String>
{
    let config_dir = master_file.parent().unwrap().join(&master.sandboxes_dir);
    let mut images = HashSet::new();
    let mut image_dirs = HashSet::new();
    let childval = ChildConfig::mapping_validator();
    try!(try!(scan_dir::ScanDir::files().read(&config_dir, |iter| {
        let yamls = iter.filter(|&(_, ref name)| name.ends_with(".yaml"));
        for (entry, tree_fname) in yamls {
            let tree_name = &tree_fname[..tree_fname.len()-5];  // strip .yaml
            let tree_config: TreeConfig = try!(parse_config(&entry.path(),
                &TreeConfig::validator(), Default::default()));
            image_dirs.insert(tree_config.image_dir.clone());

            let cfg = master_file.parent().unwrap()
                .join(&master.processes_dir)
                .join(tree_config.config_file.as_ref().unwrap_or(
                    &PathBuf::from(&(tree_name.to_string() + ".yaml"))));
            let all_children: BTreeMap<String, ChildConfig>;
            all_children = try!(parse_config(&cfg, &childval, Default::default())
                .map_err(|e| format!("Can't read child config {:?}: {}",
                                     tree_config.config_file, e)));
            for child in all_children.values() {
                // Current are always added
                images.insert(tree_config.image_dir.join(&child.image));
            }

            let logname = master.config_log_dir
                .join(format!("{}.log", tree_name));
            // TODO(tailhook) look in log rotations
            let log = try!(File::open(&logname)
                .map_err(|e| format!("Can't read log file {:?}: {}", logname, e)));
            let mut configs;
            configs = VecDeque::<(Tm, BTreeMap<String, ChildConfig>)>::new();
            for (line_no, line) in BufReader::new(log).lines().enumerate() {
                let line = try!(line
                    .map_err(|e| format!("Readline error: {}", e)));
                let mut iter = line.splitn(2, " ");
                let (tm, cfg) = match (iter.next(), iter.next()) {
                        (Some(""), None) => continue, // last line, probably
                        (Some(date), Some(config)) => {
                            // TODO(tailhook) remove or check tree name
                            (try!(time::strptime(date, "%Y-%m-%dT%H:%M:%SZ")
                                 .map_err(|_| format!("Bad time at {:?}:{}",
                                    logname, line_no))),
                             try!(json::decode(config)
                                 .map_err(|_| format!("Bad config at {:?}:{}",
                                    logname, line_no))),
                            )
                        }
                        _ => {
                            return Err(format!("Bad line at {:?}:{}",
                                               logname, line_no));
                        }
                    };
                if let Some(&(_, ref ocfg)) = configs.back(){
                    if *ocfg == cfg {
                        continue;
                    }
                }
                configs.push_back((tm, cfg));
                if configs.len() >= ver_max as usize {
                    configs.pop_front();
                }
            }
            min_time.map(|min_time| {
                while configs.len() > ver_min as usize {
                    if let Some(&(tm, _)) = configs.front() {
                        if tm > min_time {
                            break;
                        }
                    } else {
                        break;
                    }
                    configs.pop_front();
                }
            });
            for &(_, ref cfg) in configs.iter() {
                for child in cfg.values() {
                    // Current are always added
                    images.insert(tree_config.image_dir.join(&child.image));
                }
            }
        }
        Ok(())
    }).map_err(|e| format!("Read dir error: {}", e))));
    Ok((images, image_dirs))
}
