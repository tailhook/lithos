#[macro_use] extern crate log;
extern crate env_logger;
extern crate argparse;
extern crate quire;
extern crate lithos;
extern crate time;
extern crate scan_dir;
extern crate serde_json;

use std::env;
use std::rc::Rc;
use std::io::{BufReader, BufRead};
use std::fs::{File, remove_dir_all};
use std::path::{PathBuf, Path};
use std::process::exit;
use std::collections::HashSet;
use std::collections::BTreeMap;
use std::collections::VecDeque;

use time::{Tm, Duration, now_utc};
use quire::{parse_config, Options};
use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue, StoreConst};
use argparse::{Print};

use lithos::master_config::MasterConfig;
use lithos::sandbox_config::SandboxConfig;
use lithos::child_config::ChildConfig;


#[derive(Clone, Copy, Debug)]
enum Action {
    Used,
    Unused,
    DeleteUnused,
}

enum Candidate {
    Config(Tm, BTreeMap<String, ChildConfig>),
    BrokenLine(Rc<PathBuf>, usize),
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
        &MasterConfig::validator(), &Options::default()) {
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
        if !dir.exists() {
            warn!("Directory {:?} does not exists", dir);
            continue;
        }
        try!(scan_dir::ScanDir::dirs().skip_symlinks(true).read(dir, |iter| {
            for (entry, _) in iter {
                let path = entry.path().to_path_buf();
                if !used.contains(&path) {
                    unused.push(path);
                }
            }
        }));
    }
    Ok(unused)
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Candidate) -> bool {
        use Candidate::*;
        match (self, other) {
            (&BrokenLine(..), &BrokenLine(..)) => true,
            (&Config(_, ref a), &Config(_, ref b)) => a == b,
            _ => false,
        }
    }
}

fn parse_line(line: &str) -> Result<(Tm, BTreeMap<String, ChildConfig>), ()> {
    let mut iter = line.splitn(2, " ");
    let date = iter.next().ok_or(())?;
    let config = iter.next().ok_or(())?;
    Ok((
        time::strptime(date, "%Y-%m-%dT%H:%M:%SZ").map_err(|_| ())?,
        serde_json::from_str(config).map_err(|_| ())?,
    ))
}

fn find_used_images(master: &MasterConfig, master_file: &Path,
    min_time: Option<Tm>, ver_min: u32, ver_max: u32)
    -> Result<(HashSet<PathBuf>, HashSet<PathBuf>), String>
{
    let config_dir = master_file.parent().unwrap().join(&master.sandboxes_dir);
    let mut images = HashSet::new();
    let mut image_dirs = HashSet::new();
    let childval = ChildConfig::mapping_validator();
    scan_dir::ScanDir::files().read(&config_dir, |iter| {
        let yamls = iter.filter(|&(_, ref name)| name.ends_with(".yaml"));
        for (entry, sandbox_fname) in yamls {
            let sandbox_name = &sandbox_fname[..sandbox_fname.len()-5];  // strip .yaml
            let sandbox_config: SandboxConfig = parse_config(&entry.path(),
                &SandboxConfig::validator(), &Options::default())
                .map_err(|e| e.to_string())?;
            image_dirs.insert(sandbox_config.image_dir.clone());

            let cfg = master_file.parent().unwrap()
                .join(&master.processes_dir)
                .join(sandbox_config.config_file.as_ref().unwrap_or(
                    &PathBuf::from(&(sandbox_name.to_string() + ".yaml"))));
            if cfg.exists() {
                let all_children: BTreeMap<String, ChildConfig>;
                all_children =
                    parse_config(&cfg, &childval, &Options::default())
                    .map_err(|e| format!("Can't read child config {:?}: {}",
                                         sandbox_config.config_file, e))?;
                for child in all_children.values() {
                    // Current are always added
                    images.insert(sandbox_config.image_dir.join(&child.image));
                }
            } else {
                info!("No current processes for {}", sandbox_name);
            }

            let logname = Rc::new(master.config_log_dir
                .join(format!("{}.log", sandbox_name)));
            if logname.exists() {
                // TODO(tailhook) look in log rotations
                let log = try!(File::open(&*logname)
                    .map_err(|e| format!("Can't read log file {:?}: {}", logname, e)));
                let mut configs;
                configs = VecDeque::<Candidate>::new();
                for (line_no, line) in BufReader::new(log).lines().enumerate() {
                    let line = try!(line
                        .map_err(|e| format!("Readline error: {}", e)));
                    if line.trim() == "" {
                        continue; // Probably last line
                    }
                    let cand = match parse_line(&line) {
                        Ok((tm, cfg)) => Candidate::Config(tm, cfg),
                        Err(()) => {
                            warn!("Broken line {}: {}",
                                logname.display(), line_no);
                            Candidate::BrokenLine(logname.clone(), line_no)
                        }
                    };
                    if configs.back() == Some(&cand) {
                        continue;
                    }
                    configs.push_back(cand);
                    if configs.len() >= ver_max as usize {
                        configs.pop_front();
                    }
                }
                min_time.map(|min_time| {
                    while configs.len() > ver_min as usize {
                        match configs.front() {
                            Some(&Candidate::Config(tm, _)) if tm > min_time
                            => {
                                break;
                            }
                            _ => {}
                        }
                        configs.pop_front();
                    }
                });
                for cand in configs.iter() {
                    match *cand {
                        Candidate::Config(_, ref cfg) => {
                            for child in cfg.values() {
                                // Current are always added
                                images.insert(
                                    sandbox_config.image_dir
                                    .join(&child.image));
                            }
                        }
                        Candidate::BrokenLine(..) => {
                            return Err(format!("Can't reliably find out \
                                used images for sandbox {}", sandbox_name));
                        }
                    }
                }
            } else {
                info!("No log for {} probably never used", sandbox_name);
            }
        }
        Ok(())
    }).map_err(|e| format!("Read dir error: {}", e))??;
    Ok((images, image_dirs))
}
