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
use libc::pid_t;
use std::collections::{TreeMap, TreeSet};

use argparse::{ArgumentParser};

use lithos::signal;

#[allow(dead_code, unused_attribute)] mod lithos_tree;
#[allow(dead_code, unused_attribute)] mod lithos_knot;

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

fn get_tree_info(pid: pid_t) -> Result<LithosInfo, ()> {
    read_cmdline(pid)
    .map_err(|_| debug!("Can't read cmdline for {}", pid))
    .and_then(|args| {
        let mut out = MemWriter::new();
        let mut err = MemWriter::new();
        lithos_tree::Options::parse_specific_args(args, &mut out, &mut err)
            .map_err(|_| debug!("Can't parse lithos_tree cmdline for {}", pid))
    })
    .map(|opt| Tree(opt.config_file))
}

fn get_knot_info(pid: pid_t) -> Result<LithosInfo, ()> {
    read_cmdline(pid)
    .map_err(|_| debug!("Can't read cmdline for {}", pid))
    .and_then(|args| {
        let mut out = MemWriter::new();
        let mut err = MemWriter::new();
        lithos_knot::Options::parse_specific_args(args, &mut out, &mut err)
            .map_err(|_| debug!("Can't parse lithos_knot cmdline for {}", pid))
    })
    .map(|opt| Knot(opt.name))
}

fn read_process(pid: pid_t) -> Result<Process, IoError> {
    let mut f = BufferedReader::new(try!(File::open(
        &Path::new(format!("/proc/{}/status", pid).as_slice()))));
    let mut result: Process = Default::default();
    result.pid = pid;
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
                        result.lithos_info = get_tree_info(pid).ok();
                    }
                    "lithos_knot" => {
                        result.lithos_info = get_knot_info(pid).ok();
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
            for prc in children.pop(&root.pid).unwrap_or(Vec::new())
                .iter()
            {
                if let Some(Knot(ref name)) = prc.lithos_info {
                    println!(r" \--- {} {}", prc.pid, name);
                }
            }
        }
    }

    return Ok(());
}

fn main() {

    signal::block_all();

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
