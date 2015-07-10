#[macro_use] extern crate log;
extern crate env_logger;
extern crate argparse;
extern crate quire;
extern crate lithos;
extern crate time;
extern crate rustc_serialize;

use std::env;
use std::io::{BufReader, BufRead};
use std::fs::File;
use std::path::PathBuf;
use std::process::exit;
use std::collections::HashSet;
use std::collections::BTreeMap;
use std::collections::VecDeque;

use time::{Tm, Duration, now_utc};
use quire::parse_config;
use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue, StoreConst};
use rustc_serialize::json;

use lithos::utils::read_yaml_dir;
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
    let mut config_file = PathBuf::from("/etc/lithos.yaml");
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
            "Name of the global configuration file (default /etc/lithos.yaml)")
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
        ap.parse_args_or_exit();
    }
    let master: MasterConfig = match parse_config(&config_file,
        &*MasterConfig::validator(), Default::default()) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Can't parse config: {}", e);
            exit(1);
        }
    };
    let tm = days.map(|days| now_utc() - Duration::days(days as i64));
    let used_images = match find_used_images(&master, tm, ver_min, ver_max) {
        Ok(images) => images,
        Err(e) => {
            error!("Error finding out used images: {}", e);
            exit(1);
        }
    };
    println!("Used images: {:?}", used_images);
}

fn find_used_images(master: &MasterConfig, min_time: Option<Tm>,
    ver_min: u32, ver_max: u32) -> Result<HashSet<PathBuf>, String>
{
    let mut images = HashSet::new();
    let childval = &*ChildConfig::mapping_validator();
    for (tree_name, tree_fn) in try!(read_yaml_dir(&master.config_dir)
                            .map_err(|e| format!("Read dir error: {}", e)))
    {
        let tree_config: TreeConfig = try!(parse_config(&tree_fn,
            &*TreeConfig::validator(), Default::default()));
        let all_children: BTreeMap<String, ChildConfig>;
        all_children = try!(parse_config(&tree_config.config_file,
            childval, Default::default())
            .map_err(|e| format!("Can't read child config {:?}: {}",
                                 tree_config.config_file, e)));
        for child in all_children.values() {
            // Current are always added
            images.insert(tree_config.image_dir.join(&child.image));
        }
        let logname = master.config_log_dir.join(format!("{}.log", tree_name));
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
    Ok(images)
}