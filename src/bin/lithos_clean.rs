extern crate argparse;
extern crate env_logger;
extern crate humantime;
extern crate lithos;
extern crate quire;
extern crate scan_dir;
extern crate serde_json;
extern crate time;
#[macro_use] extern crate log;
#[macro_use] extern crate matches;

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::env;
use std::fs::{File, remove_dir_all, remove_file};
use std::io::{self, BufReader, BufRead};
use std::path::{PathBuf, Path};
use std::process::exit;
use std::rc::Rc;
use std::time::{SystemTime, Duration, UNIX_EPOCH};

use quire::{parse_config, Options};
use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue, StoreConst};
use argparse::{Print, StoreOption};

use lithos::child_config::ChildConfig;
use lithos::master_config::MasterConfig;
use lithos::MAX_CONFIG_LOGS;
use lithos::sandbox_config::SandboxConfig;


#[derive(Clone, Copy, Debug)]
enum Action {
    Used,
    Unused,
    DeleteUnused,
}

enum Candidate {
    Config(SystemTime, BTreeMap<String, ChildConfig>),
    BrokenLine(Rc<PathBuf>, usize),
}

struct LogFiles<'a> {
    base: &'a Path,
    name: String,
    idx: u32,
}

struct ScanResult {
    images: HashSet<PathBuf>,
    image_dirs: HashSet<PathBuf>,
    unused_logs: Vec<PathBuf>,
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
    let mut clean_logs = false;
    let mut days = None::<u32>;
    let mut keep_recent = None::<humantime::Duration>;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Show used/unused images and clean if needed");
        ap.refer(&mut config_file)
          .add_option(&["-C", "--config"], Parse,
            "Name of the global configuration file \
             (default /etc/lithos/master.yaml)")
          .metavar("FILE");
        ap.refer(&mut keep_recent)
          .add_option(&["--keep-recent"], StoreOption,
            "Keep directories having recently changed ctime or mtime.
             This option is useful to remove race condition between
             uploading an image and deleting it. For example,
             ``--keep-recent=1h`` would not delete directories created within
             1 hour from now.")
          .metavar("DELTA");
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
        ap.refer(&mut clean_logs)
          .add_option(&["--clean-logs"], StoreConst(true),
            "In combination with `--unused` shows unused logs, \
             in combination with `--delete-unused` deletes them.");
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
    let tm = days.map(|days| {
        SystemTime::now() - Duration::new((days*86400) as u64, 0)
    });
    let scan_result = match find_used_images(&master, &config_file,
        tm, ver_min, ver_max)
    {
        Ok(scan_result) => scan_result,
        Err(e) => {
            error!("Error finding out used images: {}", e);
            exit(1);
        }
    };
    match action {
        Action::Used => {
            for i in &scan_result.images {
                println!("{:?}", i);
            }
        }
        Action::Unused => {
            let unused = find_unused(&scan_result.images,
                                     &scan_result.image_dirs,
                                     keep_recent.map(|x| *x))
                .map_err(|e| {
                    error!("Error finding unused images: {:?}", e);
                    exit(2);
                })
                .unwrap();
            for i in unused {
                println!("{:?}", i);
            }
            if clean_logs {
                for log in scan_result.unused_logs {
                    if log.exists() {
                        println!("{:?}", log);
                    }
                }
            }
        }
        Action::DeleteUnused => {
            let unused = find_unused(&scan_result.images,
                                     &scan_result.image_dirs,
                                     keep_recent.map(|x| *x))
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
            if clean_logs {
                for log in scan_result.unused_logs {
                    if verbose {
                        println!("Deleting log {:?}", log);
                    }
                    match remove_file(&log) {
                        Ok(()) => {}
                        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
                        Err(ref e) => {
                            error!("Can't remove log {:?}: {}", log, e);
                        }
                    }
                }
            }
        }
    }
}

impl<'a> LogFiles<'a> {
    fn new(path: &'a Path, name: &'a str) -> LogFiles<'a> {
        LogFiles {
            base: path,
            name: format!("{}.log", name),
            idx: 0,
        }
    }
}

impl<'a> Iterator for LogFiles<'a> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> {
        if self.idx == 0 {
            let result = self.base.join(&self.name);
            self.idx += 1;
            debug!("Trying {:?}: {}", result, result.exists());
            if result.exists() {
                return Some(result);
            }
        }
        while self.idx < MAX_CONFIG_LOGS {
            let result = self.base.join(format!("{}.{}", self.name, self.idx));
            self.idx += 1;
            debug!("Trying {:?}: {}", result, result.exists());
            if result.exists() {
                return Some(result);
            }
        }
        return None;
    }
}

fn find_unused(used: &HashSet<PathBuf>, dirs: &HashSet<PathBuf>,
               keep_recent: Option<Duration>)
    -> Result<Vec<PathBuf>, scan_dir::Error>
{
    let cut_off = keep_recent.map(|x| SystemTime::now() - x);
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
                    if let Some(cut) = cut_off {
                        match path.metadata() {
                            Ok(m) => {
                                let skip = m.created().map(|x| x > cut)
                                    // allow FSs with no `birthtime`
                                    .unwrap_or(false) ||
                                    m.modified().map(|x| x > cut)
                                    .map_err(|e| error!(
                                        "Can't read mtime {:?}: {}", path, e))
                                    // no `mtime` is something wrong
                                    // skip it for safety
                                    .unwrap_or(true);
                                if skip {
                                    continue;
                                }
                            }
                            Err(e) => {
                                error!("Can't stat {:?}: {}", path, e);
                                continue;
                            }
                        }
                    }
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

fn parse_line(line: &str)
    -> Result<(SystemTime, BTreeMap<String, ChildConfig>), ()>
{
    let mut iter = line.splitn(2, " ");
    let date = iter.next().ok_or(())?;
    let config = iter.next().ok_or(())?;
    let time = time::strptime(date, "%Y-%m-%dT%H:%M:%SZ").map_err(|_| ())?;
    Ok((
        UNIX_EPOCH + Duration::new(time.to_timespec().sec as u64, 0),
        serde_json::from_str(config).map_err(|_| ())?,
    ))
}

fn find_used_by_list(_master: &MasterConfig, _sandbox_name: &str,
    sandbox_config: &SandboxConfig,
    images: &mut HashSet<PathBuf>, bad_dirs: &mut HashSet<PathBuf>)
{
    let ref filename = sandbox_config.used_images_list.as_ref().unwrap();
    let log = match File::open(filename) {
        Ok(f) => BufReader::new(f),
        Err(e) => {
            error!("Can't read image list {:?}: {}", filename, e);
            bad_dirs.insert(sandbox_config.image_dir.clone());
            return;
        }
    };
    for line in log.lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                error!("Can't read image list {:?}: {}", filename, e);
                bad_dirs.insert(sandbox_config.image_dir.clone());
                return;
            }
        };
        let image_name = line.trim();
        if image_name.len() > 0 {
            images.insert(Path::new(image_name).into());
        }
    }
}

fn find_used_by_log(master: &MasterConfig, sandbox_name: &str,
    sandbox_config: &SandboxConfig,
    min_time: Option<SystemTime>, ver_min: u32, ver_max: u32,
    images: &mut HashSet<PathBuf>, unused_logs: &mut Vec<PathBuf>,
    bad_dirs: &mut HashSet<PathBuf>)
{
    let mut configs = VecDeque::<Candidate>::new();
    let mut log_iter = LogFiles::new(
        master.config_log_dir.as_ref().unwrap(), &sandbox_name);
    for logname in log_iter.by_ref() {
        let logname = Rc::new(logname);
        let log = match File::open(&*logname) {
            Ok(f) => f,
            Err(e) => {
                error!("Can't read log file {:?}: {}", logname, e);
                bad_dirs.insert(sandbox_config.image_dir.clone());
                return;
            }
        };

        for (line_no, line) in BufReader::new(log).lines().enumerate() {
            let line = match line {
                Ok(f) => f,
                Err(e) => {
                    error!("Readline error {:?}: {}", logname, e);
                    bad_dirs.insert(sandbox_config.image_dir.clone());
                    return;
                }
            };
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
            if configs.len() as u32 > ver_max {
                configs.pop_front();
            }
        }
        if configs.len() as u32 >= ver_max ||
            matches!((configs.front(), min_time),
                (Some(&Candidate::Config(tm, _)), Some(min_time))
                                      if tm > min_time)
        {
            break;
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
                    images.insert(
                        sandbox_config.image_dir
                        .join(&child.image));
                }
            }
            Candidate::BrokenLine(..) => {
                bad_dirs.insert(sandbox_config.image_dir.clone());
            }
        }
    }
    for logname in log_iter {
        unused_logs.push(logname);
    }
}

fn find_used_images(master: &MasterConfig, master_file: &Path,
    min_time: Option<SystemTime>, ver_min: u32, ver_max: u32)
    -> Result<ScanResult, String>
{
    let config_dir = master_file.parent().unwrap().join(&master.sandboxes_dir);
    let mut bad_dirs = HashSet::new();
    let mut images = HashSet::new();
    let mut image_dirs = HashSet::new();
    let mut unused_logs = Vec::new();
    let childval = ChildConfig::mapping_validator();
    scan_dir::ScanDir::files().read(&config_dir, |iter| -> Result<(), String> {
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

            if sandbox_config.used_images_list.is_some() {
                find_used_by_list(master, sandbox_name, &sandbox_config,
                    &mut images, &mut bad_dirs);
            } else if master.config_log_dir.is_some() {
                find_used_by_log(master, sandbox_name, &sandbox_config,
                    min_time, ver_min, ver_max,
                    &mut images, &mut unused_logs, &mut bad_dirs);
            } else {
                error!("Neither `config-log-dir` nor `used-images-list` is \
                        set for sandbox {:?}. Can't clean images.",
                        sandbox_name);
            }
        }
        Ok(())
    }).map_err(|e| format!("Read dir error: {}", e))??;

    for dir in &bad_dirs {
        error!("Can't reliably find out used images in the directory {:?}",
            dir);
        image_dirs.remove(dir);
    }
    Ok(ScanResult { images, image_dirs, unused_logs })
}
