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
use std::io::{stdout, stderr};
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
use serialize::json;

use argparse::{ArgumentParser, StoreConst};

use ascii::Printer;
use lithos::signal;

#[allow(dead_code, unused_attribute)] mod lithos_tree;
#[allow(dead_code, unused_attribute)] mod lithos_knot;

#[path = "../ascii.rs"] mod ascii;

static mut boot_time: u64 = 0;
static mut clock_ticks: u64 = 100;

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
    start_time: u64,
    mem_rss: uint,
    mem_swap: uint,
    user_time: u64,
    system_time: u64,
    child_user_time: u64,
    child_system_time: u64,
    threads: uint,
    cmdline: Vec<String>,
}

struct ScanResult {
    children: TreeMap<pid_t, Vec<Rc<Process>>>,
    roots: TreeSet<Rc<Process>>,
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
    let stat_re = regex!(
        // We parse bracketed executable and double-bracketed, still
        // if executable name itself contains bracket we would fail
        // But even if we fail, we get only start_time missed or incorrect
        concat!(r"^\d+",
            r"\s+(?:\([^)]*\)|\(\([^)]*\)\))(?:\s+(\S+)){12}",
            r"\s+(?P<utime>\d+)\s+(?P<stime>\d+)",
            r"\s+(?P<cutime>\d+)\s+(?P<cstime>\d+)",
            r"(?:\s+(\S+)){4}",
            r"\s+(?P<start_time>\d+)\b"));
        // TODO(tailhook) we can get executable name from /status and match
        // it here, the executable in /status is escaped!
    match stat_re.captures(line.as_slice()) {
        Some(c) => {
            FromStr::from_str(c.name("start_time"))
                .map(|v| result.start_time = v);
            FromStr::from_str(c.name("utime"))
                .map(|v| result.user_time = v);
            FromStr::from_str(c.name("stime"))
                .map(|v| result.system_time = v);
            FromStr::from_str(c.name("cutime"))
                .map(|v| result.child_user_time = v);
            FromStr::from_str(c.name("cstime"))
                .map(|v| result.child_system_time = v);
        }
        None => {
            warn!("Error getting start_time for pid {}", pid);
        }
    }

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

fn start_time_sec(start_ticks: u64) -> u64 {
    return unsafe { boot_time  + (start_ticks / clock_ticks) };
}

fn format_uptime(start_ticks: u64) -> String {
    let start_time = start_time_sec(start_ticks);
    let uptime = get_time().sec as u64 - start_time;
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

fn subtree_name(fullname: &String) -> Option<String> {
    fullname.as_slice().as_slice().splitn(1, '/').next().map(|x| x.to_string())
}

fn scan_processes() -> Result<ScanResult, IoError>
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
    return Ok(ScanResult {
        children: children,
        roots: roots,
    });
}

fn print_tree(scan: ScanResult) -> Result<(), IoError> {
    let children = scan.children;
    let roots = scan.roots;

    let mut trees: Vec<ascii::TreeNode> = vec!();
    let mut out = stdout();

    for root in roots.iter() {
        if let Some(Tree(ref cfg_file)) = root.lithos_info {
            let mut subtrees = TreeMap::new();
            for prc in children.find(&root.pid).unwrap_or(&Vec::new())
                .iter()
            {
                let name = if let Some(Knot(ref name)) = prc.lithos_info {
                    name
                } else {
                    continue;
                };
                let subname = if let Some(subname) = subtree_name(name) {
                    subname
                } else {
                    warn!("Wrong child name {}", name);
                    continue;
                };
                let mut subtree = subtrees.pop(&subname).unwrap_or(vec!());

                if let Some(knot_children) = children.find(&prc.pid) {
                    if knot_children.len() == 1 {
                        let child = &knot_children[0];
                        let mut info: KnotTotals = Default::default();
                        info._add_process(&**child, &children);
                        subtree.push(ascii::TreeNode {
                            head: ascii::ColorPrinter("".to_string())
                                .green(&child.pid)
                                .norm(name)
                                .blue(&format_uptime(child.start_time))
                                .blue(&format!("[{}/{}]",
                                              info.processes,
                                              info.threads))
                                .blue(&format_memory(
                                    info.mem_rss + info.mem_swap))
                                .unwrap(),
                            children: vec!(),
                        });
                    } else {
                        let mut processes = vec!();
                        for child in knot_children.iter() {
                            let mut info: KnotTotals = Default::default();
                            info._add_process(&**child, &children);
                            processes.push(ascii::TreeNode {
                                head: ascii::ColorPrinter("".to_string())
                                    .green(&child.pid)
                                    .norm(name)
                                    .blue(&format_uptime(child.start_time))
                                    .blue(&format!("[{}/{}]",
                                                  info.processes,
                                                  info.threads))
                                    .blue(&format_memory(
                                        info.mem_rss + info.mem_swap))
                                    .unwrap(),
                                children: vec!(),
                            });
                        }
                        subtree.push(ascii::TreeNode {
                            head: ascii::ColorPrinter("".to_string())
                                .red(&format!("({})", prc.pid))
                                .norm(name)
                                .blue(&format!("[multiple]"))
                                .unwrap(),
                            children: processes,
                        });
                    }
                } else {
                    subtree.push(ascii::TreeNode {
                        head: ascii::ColorPrinter("".to_string())
                            .red(&format!("({})", prc.pid))
                            .norm(name)
                            .red(&format!("[failing]"))
                            .unwrap(),
                        children: vec!(),
                    });
                }
                subtrees.insert(subname, subtree);
            }
            trees.push(ascii::TreeNode {
                head: ascii::ColorPrinter("".to_string())
                    .blue(&root.pid)
                    .norm(&"tree".to_string())
                    .blue(&cfg_file.display())
                    .unwrap(),
                children: subtrees.into_iter().map(|(key, val)|
                    ascii::TreeNode {
                        head: key,
                        children: val,
                    }).collect(),
            });
        }
    }

    for tree in trees.iter() {
        try!(tree.print(&mut out));
    }

    return Ok(());
}

fn print_json(scan: ScanResult) -> Result<(), IoError> {
    let children = scan.children;
    let roots = scan.roots;
    let mut trees = vec!();

    for root in roots.iter() {
        let cfg_file = if let Some(Tree(ref cfg_file)) = root.lithos_info {
            cfg_file
        } else {
            continue;
        };
        let mut knots = vec!();
        for prc in children.find(&root.pid).unwrap_or(&Vec::new()).iter() {
            let mut processes = vec!();
            if let Some(knot_children) = children.find(&prc.pid) {
                for child in knot_children.iter() {
                    let mut info: KnotTotals = Default::default();
                    info._add_process(&**child, &children);
                    processes.push(json::Object(vec!(
                        ("pid".to_string(), json::U64(prc.pid as u64)),
                        ("processes".to_string(),
                            json::U64(info.processes as u64)),
                        ("threads".to_string(),
                            json::U64(info.threads as u64)),
                        ("mem_rss".to_string(),
                            json::U64(info.mem_rss as u64)),
                        ("mem_swap".to_string(),
                            json::U64(info.mem_swap as u64)),
                        ("start_time".to_string(),
                            json::U64(start_time_sec(child.start_time))),
                        ("user_time".to_string(),
                            json::U64(child.user_time)),
                        ("system_time".to_string(),
                            json::U64(child.system_time)),
                        ("child_user_time".to_string(),
                            json::U64(child.child_user_time)),
                        ("child_system_time".to_string(),
                            json::U64(child.child_system_time)),
                        ).into_iter().collect()));
                }
            }
            knots.push(json::Object(vec!(
                ("pid".to_string(), json::U64(prc.pid as u64)),
                ("ok".to_string(), json::Boolean(processes.len() == 1)),
                ("processes".to_string(), json::List(processes)),
                ).into_iter().collect()));
        }
        trees.push(json::Object(vec!(
            ("pid".to_string(), json::U64(root.pid as u64)),
            ("config".to_string(),
                json::String(cfg_file.display().to_string())),
            ("children".to_string(),
                json::List(knots)),
            ).into_iter().collect()));
    }


    let mut out = stdout();
    return out.write_str(json::encode(&json::Object(vec!(
        ("trees".to_string(), json::List(trees)),
        ).into_iter().collect())).as_slice());
}

fn read_global_consts() {
    unsafe {
        clock_ticks = sysconf(_SC_CLK_TCK) as u64;
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

    let mut action = print_tree;

    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut action)
            .add_option(["--json"], box StoreConst(print_json),
                "Print big json instead human-readable tree");
            .add_option(["--monitor"], box StoreConst(print_json),
                "Print big json instead human-readable tree");
        ap.set_description("Displays tree of processes");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match scan_processes().and_then(|s| action(s)) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
