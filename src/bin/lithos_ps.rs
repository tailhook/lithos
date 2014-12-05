#![feature(phase, macro_rules, if_let, slicing_syntax)]

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


use std::rc::Rc;
use std::os::args;
use std::io::stderr;
use std::io::IoError;
use std::io::fs::File;
use std::from_str::FromStr;
use std::io::fs::{readdir};
use std::io::{BufferedReader, MemWriter};
use std::os::{set_exit_status};
use std::default::Default;
use std::collections::{TreeMap, TreeSet};
use time::get_time;
use libc::pid_t;
use libc::consts::os::sysconf::_SC_CLK_TCK;
use libc::funcs::posix88::unistd::sysconf;

use argparse::{ArgumentParser};

use lithos::signal;

#[allow(dead_code, unused_attribute)] mod lithos_tree;
#[allow(dead_code, unused_attribute)] mod lithos_knot;

#[path = "../ascii.rs"] mod ascii;

static mut boot_time: uint = 0;
static mut clock_ticks: uint = 100;

#[deriving(PartialEq, Eq, PartialOrd, Ord)]
enum LithosInfo {
    Tree(Path),
    Knot(String),
}

#[deriving(Default, PartialEq, Eq, PartialOrd, Ord)]
struct Process {
    parent_id: pid_t,
    pid: pid_t,
    lithos_info: Option<LithosInfo>,
    start_time: uint,
    mem_rss: uint,
    mem_swap: uint,
    threads: uint,
    cmdline: Vec<String>,
}

#[deriving(Default)]
struct KnotTotals {
    mem_rss: uint,
    mem_swap: uint,
    processes: uint,
    threads: uint,
}

fn parse_mem_size(value: &str) -> uint {
    let mut pair = value.as_slice().splitn(1, ' ');
    let val: uint = FromStr::from_str(pair.next().unwrap())
        .expect("Memory should be integer");
    let unit = pair.next().unwrap_or("kB");
    match unit {
        "kB" | "" => val * 1024,
        "MB" => val * 1024 * 1024,
        _ => {
            warn!("Wrong memory unit: {}", unit);
            val * 1024
        }
    }
}

fn read_cmdline(pid: pid_t) -> Result<Vec<String>, IoError> {
    let mut f = try!(File::open(
        &Path::new(format!("/proc/{}/cmdline", pid).as_slice())));
    let mut args: Vec<String> = try!(f.read_to_string())
              .as_slice().split('\0')
              .map(|x| x.to_string())
              .collect();
    args.pop();  // empty arg at the end
    return Ok(args);
}

fn get_tree_info(pid: pid_t, cmdline: &Vec<String>) -> Result<LithosInfo, ()> {
    let args = cmdline.clone();
    let mut out = MemWriter::new();
    let mut err = MemWriter::new();
    lithos_tree::Options::parse_specific_args(args, &mut out, &mut err)
        .map(|opt| Tree(opt.config_file))
        .map_err(|_| debug!("Can't parse lithos_tree cmdline for {}", pid))
}

fn get_knot_info(pid: pid_t, cmdline: &Vec<String>) -> Result<LithosInfo, ()> {
    let args = cmdline.clone();
    let mut out = MemWriter::new();
    let mut err = MemWriter::new();
    lithos_knot::Options::parse_specific_args(args, &mut out, &mut err)
        .map(|opt| Knot(opt.name))
        .map_err(|_| debug!("Can't parse lithos_knot cmdline for {}", pid))
}

fn read_process(pid: pid_t) -> Result<Process, IoError> {
    let mut f = BufferedReader::new(try!(File::open(
        &Path::new(format!("/proc/{}/status", pid).as_slice()))));
    let mut result: Process = Default::default();
    result.pid = pid;
    result.cmdline = try!(read_cmdline(pid));
    for line in f.lines() {
        let line = try!(line);
        let mut pair = line.as_slice().splitn(1, ':');
        let name = pair.next().unwrap().trim();
        let value = pair.next().expect("Colon and value expected").trim();
        match name.as_slice(){
            "PPid" => {
                result.parent_id = FromStr::from_str(value)
                    .expect("Ppid should be integer");
            }
            "VmRSS" => {
                result.mem_rss = parse_mem_size(value);
            }
            "VmSwap" => {
                result.mem_swap = parse_mem_size(value);
            }
            "Threads" => {
                result.threads = FromStr::from_str(value)
                    .expect("Threads should be integer");
            }
            "Name" => {
                match value {
                    "lithos_tree" => {
                        result.lithos_info = get_tree_info(
                            pid, &result.cmdline).ok();
                    }
                    "lithos_knot" => {
                        result.lithos_info = get_knot_info(
                            pid, &result.cmdline).ok();
                    }
                    _ => {}
                };
            }
            _ => {}
        }
    }
    let mut f = BufferedReader::new(try!(File::open(
        &Path::new(format!("/proc/{}/stat", pid).as_slice()))));
    let line = try!(f.read_line());
    let starttime_re = regex!(
        // We parse bracketed executable and double-bracketed, still
        // if executable name itself contains bracket we would fail
        // But even if we fail, we get only start_time missed or incorrect
        r"^\d+\s+(?:\([^)]*\)|\(\([^)]*\)\))(?:\s+(\S+)){20}\s+(\d+)\b");
        // TODO(tailhook) we can get executable name from /status and match
        // it here, the executable in /status is escaped!
    result.start_time = starttime_re
        .captures(line.as_slice())
        .map(|c| c.at(1))
        .and_then(|v| FromStr::from_str(v))
        .unwrap_or_else(|| {
            warn!("Error getting start_time for pid {}", pid);
            return 0;
        });

    return Ok(result);
}

impl KnotTotals {
    fn _add_process(&mut self, prc: &Process,
        children: &TreeMap<pid_t, Vec<Rc<Process>>>)
    {
        self.mem_rss += prc.mem_rss;
        self.mem_swap += prc.mem_swap;
        self.processes += 1;
        self.threads += prc.threads;
        if let Some(ref lst) = children.find(&prc.pid) {
            for prc in lst.iter() {
                self._add_process(&**prc, children);
            }
        }
    }
}

fn format_uptime(start_ticks: uint) -> String {
    let start_time = unsafe { boot_time  + (start_ticks / clock_ticks) };
    let uptime = get_time().sec as uint - start_time;
    if uptime < 60 {
        format!("{}s", uptime)
    } else if uptime < 3600 {
        format!("{}m{}s", uptime / 60, uptime % 60)
    } else if uptime < 86400 {
        format!("{}h{}m{}s", uptime / 3600, (uptime / 60) % 60, uptime % 60)
    } else if uptime < 3*86400 {
        format!("{}d{}h{}m", uptime / 86400,
            (uptime / 3600) % 24, (uptime / 60) % 60)
    } else {
        format!("{}days", uptime / 86400)
    }
}

fn format_memory(mem: uint) -> String {
    if mem < (1 << 10) {
        format!("{}B", mem)
    } else if mem < (1 << 20) {
        format!("{:.1f}kiB", mem as f64/(1024_f64))
    } else if mem < (1 << 30) {
        format!("{:.1f}MiB", mem as f64/(1_048_576_f64))
    } else {
        format!("{:.1f}GiB", mem as f64/(1_048_576_f64 * 1024_f64))
    }
}

fn scan_processes() -> Result<(), IoError>
{
    let mut children = TreeMap::<pid_t, Vec<Rc<Process>>>::new();
    let mut roots = TreeSet::<Rc<Process>>::new();

    for pid in try!(readdir(&Path::new("/proc")))
        .into_iter()
        .filter_map(|p| p.filename_str().and_then(FromStr::from_str))
    {
        match read_process(pid) {
            Ok(prc) => {
                let prc = Rc::new(prc);
                if let Some(Tree(_)) = prc.lithos_info {
                    roots.insert(prc.clone());
                }
                if let Some(vec) = children.find_mut(&prc.parent_id) {
                    vec.push(prc);
                    continue;
                }
                children.insert(prc.parent_id, vec!(prc));
            }
            Err(e) => {
                info!("Error reading pid {}: {}", pid, e);
            }
        }
    }

    for root in roots.iter() {
        if let Some(Tree(ref cfg_file)) = root.lithos_info {
            println!("-+= {} tree ({})", root.pid, cfg_file.display());
            for prc in children.find(&root.pid).unwrap_or(&Vec::new())
                .iter()
            {
                if let Some(Knot(ref name)) = prc.lithos_info {
                    if let Some(knot_children) = children.find(&prc.pid) {
                        if knot_children.len() == 1 {
                            let child = &knot_children[0];
                            let mut info: KnotTotals = Default::default();
                            info._add_process(&**child, &children);
                            println!(r" \--- {} {} {} [{}/{}] {}",
                                child.pid, name,
                                format_uptime(child.start_time),
                                info.processes,
                                info.threads,
                                format_memory(info.mem_rss + info.mem_swap));
                        } else {
                            println!(r" \-+- ({}) {} [multiple]",
                                     prc.pid, name);
                            for child in knot_children.iter() {
                                let mut info: KnotTotals = Default::default();
                                info._add_process(&**child, &children);
                                println!(r"   \--- {} {} {} [{}/{}] {}",
                                    child.pid, name,
                                    format_uptime(child.start_time),
                                    info.processes,
                                    info.threads,
                                    format_memory(info.mem_rss+info.mem_swap),
                                    );
                            }
                        }
                    } else {
                            println!(r" \--- ({}) {} [failing]",
                                     prc.pid, name);
                    }
                }
            }
        }
    }

    return Ok(());
}

fn read_global_consts() {
    unsafe {
        clock_ticks = sysconf(_SC_CLK_TCK) as uint;
        boot_time =
            BufferedReader::new(
                File::open(&Path::new("/proc/stat"))
                .ok().expect("Can't read /proc/stat"))
            .lines()
            .map(|line| line.ok().expect("Can't read /proc/stat"))
            .filter(|line| line.as_slice().starts_with("btime "))
            .next()
            .and_then(|line| FromStr::from_str(line.as_slice()[5..].trim()))
            .expect("No boot time in /proc/stat");
    }
}

fn main() {

    signal::block_all();

    read_global_consts();

    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Displays tree of processes");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match scan_processes() {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
