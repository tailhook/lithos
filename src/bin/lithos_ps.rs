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
use std::mem::swap;
use std::io::fs::File;
use std::io::timer::sleep;
use std::from_str::FromStr;
use std::io::fs::{readdir};
use std::io::{BufferedReader, MemWriter};
use std::os::{set_exit_status};
use std::default::Default;
use std::collections::{TreeMap, TreeSet};
use std::time::duration::Duration;
use time::get_time;
use libc::pid_t;
use libc::consts::os::sysconf::_SC_CLK_TCK;
use libc::funcs::posix88::unistd::sysconf;
use serialize::json;

use argparse::{ArgumentParser, StoreConst};

#[allow(dead_code, unused_attribute)] mod lithos_tree;
#[allow(dead_code, unused_attribute)] mod lithos_knot;

#[path = "../ascii.rs"] mod ascii;

static mut boot_time: u64 = 0;
static mut clock_ticks: u64 = 100;

struct Options {
    printer_factory: ascii::PrinterFactory,
}

#[deriving(PartialEq, Eq, PartialOrd, Ord)]
enum LithosInfo {
    TreeInfo(Path),
    KnotInfo(String, String, uint),
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

#[allow(dead_code)]  // sub-groups are unused, but will be in future
struct Group {
    totals: GroupTotals,
    head: Rc<Process>,
    groups: Vec<Group>,
}

#[allow(dead_code)]  // index is not used yet
struct Instance {
    name: String,
    index: uint,
    knot_pid: i32,
    totals: GroupTotals,
    heads: Vec<Group>,
}

#[deriving(Default)]
struct Child {
    totals: GroupTotals,
    instances: TreeMap<uint, Instance>,
}

#[deriving(Default)]
struct Tree {
    totals: GroupTotals,
    children: TreeMap<String, Child>,
}

struct Master {
    pid: pid_t,
    config: Path,
    totals: GroupTotals,
    trees: TreeMap<String, Tree>,
}

#[allow(dead_code)]  // totals will be used soon
struct ScanResult {
    masters: TreeMap<pid_t, Master>,
    totals: GroupTotals,
}

#[deriving(Default)]
struct GroupTotals {
    processes: uint,
    threads: uint,
    memory: uint,
    mem_rss: uint,
    mem_swap: uint,
    cpu_time: u64,
    user_time: u64,
    system_time: u64,
    child_user_time: u64,
    child_system_time: u64,
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
        .map(|opt| TreeInfo(opt.config_file))
        .map_err(|_| debug!("Can't parse lithos_tree cmdline for {}", pid))
}

fn get_knot_info(pid: pid_t, cmdline: &Vec<String>) -> Result<LithosInfo, ()> {
    let fullname_re = regex!(r"^([\w-]+)/([\w-]+)\.(\d+)$");
    let args = cmdline.clone();
    let mut out = MemWriter::new();
    let mut err = MemWriter::new();
    let opt = try!(
        lithos_knot::Options::parse_specific_args(args, &mut out, &mut err)
        .map_err(|_| debug!("Can't parse lithos_knot cmdline for {}", pid)));
    fullname_re.captures(opt.name.as_slice())
        .map(|c| KnotInfo(
            c.at(1).to_string(),
            c.at(2).to_string(),
            FromStr::from_str(c.at(3)).unwrap()
        ))
        .ok_or(())
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
            r"\s+(?:\([^)]*\)|\(\([^)]*\)\))(?:\s+\S+){11}",
            r"\s+(?P<utime>\d+)\s+(?P<stime>\d+)",
            r"\s+(?P<cutime>\d+)\s+(?P<cstime>\d+)",
            r"(?:\s+\S+){4}",
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

impl GroupTotals {
    fn new(prc: &Process) -> GroupTotals {
        return GroupTotals {
            processes: 1,
            threads: prc.threads,
            memory: prc.mem_rss + prc.mem_swap,
            mem_rss: prc.mem_rss,
            mem_swap: prc.mem_swap,
            cpu_time: prc.user_time + prc.system_time +
                      prc.child_user_time + prc.child_system_time,
            user_time: prc.user_time,
            system_time: prc.system_time,
            child_user_time: prc.child_user_time,
            child_system_time: prc.child_system_time,
        };
    }
    fn add_group(&mut self, group: &GroupTotals) {
        self.processes += group.processes;
        self.threads += group.threads;
        self.memory += group.memory;
        self.mem_rss += group.mem_rss;
        self.mem_swap += group.mem_swap;
        self.cpu_time += group.cpu_time;
        self.user_time += group.user_time;
        self.system_time += group.system_time;
        self.child_user_time += group.child_user_time;
        self.child_system_time += group.child_system_time;
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

fn _scan_group(head: Rc<Process>,
    all_children: &TreeMap<pid_t, Vec<Rc<Process>>>)
    -> Group
{
    let mut totals = GroupTotals::new(&*head);
    let mut groups = vec!();
    if let Some(children) = all_children.find(&head.pid) {
        for child in children.iter() {
            let grp = _scan_group(child.clone(), all_children);
            totals.add_group(&grp.totals);
            groups.push(grp);
        }
    }
    return Group {
        totals: totals,
        head: head,
        groups: groups,
    };
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
                if let Some(TreeInfo(_)) = prc.lithos_info {
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

    let mut masters = TreeMap::new();
    let mut totals: GroupTotals = Default::default();

    for root in roots.iter() {
        let cfg_file = if let Some(TreeInfo(ref cfg_file)) = root.lithos_info {
            cfg_file
        } else {
            continue;
        };
        let mut trees = TreeMap::<String, Tree>::new();
        let mut mtotals: GroupTotals = Default::default();
        for prc in children.find(&root.pid).unwrap_or(&Vec::new()).iter() {
            if let Some(KnotInfo(ref sub, ref name, idx)) = prc.lithos_info {
                let mut heads = vec!();
                let mut ktotals: GroupTotals = Default::default();
                if let Some(knot_children) = children.find(&prc.pid) {
                    for child in knot_children.iter() {
                        let grp = _scan_group(child.clone(), &children);
                        ktotals.add_group(&grp.totals);
                        heads.push(grp);
                    }
                }
                mtotals.add_group(&ktotals);
                if !trees.contains_key(sub) {
                    trees.insert(sub.clone(), Default::default());
                }
                trees.find_mut(sub).map(|ref mut tree| {
                    tree.totals.add_group(&ktotals);
                    if !tree.children.contains_key(name) {
                        tree.children.insert(name.clone(), Default::default());
                    }
                    tree.children.find_mut(name).map(|ref mut child| {
                        let mut nheads = vec!();
                        swap(&mut nheads, &mut heads);
                        child.totals.add_group(&ktotals);
                        child.instances.insert(idx, Instance {
                            name: format!("{}/{}.{}", sub, name, idx),
                            knot_pid: prc.pid,
                            index: idx,
                            totals: ktotals,
                            heads: nheads,
                        });
                    });
                });
            }
        }
        totals.add_group(&mtotals);
        masters.insert(root.pid, Master {
            pid: root.pid,
            config: cfg_file.clone(),
            trees: trees,
            totals: mtotals,
            });
    }

    return Ok(ScanResult {
        masters: masters,
        totals: totals,
    });
}

fn print_instance(inst: &Instance, opt: &Options) -> ascii::TreeNode {
    let label = if inst.heads.len() == 1 {
        let ref prc = inst.heads[0].head;
        opt.printer_factory.new()
            .green(&prc.pid)
            .norm(&inst.name)
            .blue(&format_uptime(prc.start_time))
            .blue(&format!("[{}/{}]",
                           inst.totals.processes,
                           inst.totals.threads))
            .blue(&format_memory(inst.totals.memory))
            .unwrap()
    } else {
        let mut prn = opt.printer_factory.new()
            .red(&format!("({})", inst.knot_pid))
            .norm(&inst.name);
        if inst.heads.len() == 0 {
            prn = prn.red(&"<failing>".to_string());
        } else {
            prn = prn
                .red(&format!("<{}>", inst.heads.len()))
                .blue(&format!("[{}/{}]",
                               inst.totals.processes,
                               inst.totals.threads))
                .blue(&format_memory(inst.totals.memory))
        }
        prn.unwrap()
    };
    return ascii::TreeNode {
        head: label,
        children: vec!(),  // TODO(tailhook) deeper aggregation
    };
}

fn print_child(name: &String, child: &Child, opt: &Options)
    -> ascii::TreeNode
{
    return ascii::TreeNode {
        head: opt.printer_factory.new()
            .norm(name)
            .blue(&format!("[{}/{}]",
                           child.totals.processes,
                           child.totals.threads))
            .blue(&format_memory(child.totals.memory))
            .unwrap(),
        children: child.instances.iter()
                  .map(|(_, inst)| print_instance(inst, opt))
                  .collect(),
    };
}

fn print_tree(name: &String, tree: &Tree, opt: &Options) -> ascii::TreeNode {
    let children = tree.children.iter()
                  .map(|(name, child)| print_child(name, child, opt))
                  .collect();
    return ascii::TreeNode {
        head: opt.printer_factory.new()
            .norm(name)
            .blue(&format!("[{}/{}]",
                           tree.totals.processes,
                           tree.totals.threads))
            .blue(&format_memory(tree.totals.memory))
            .unwrap(),
        children: children,
    };
}

fn print_full_tree(scan: ScanResult, opt: &Options) -> Result<(), IoError> {

    let mut trees: Vec<ascii::TreeNode> = vec!();

    for (pid, ref master) in scan.masters.iter() {
        trees.push(ascii::TreeNode {
            head: opt.printer_factory.new()
                .blue(pid)
                .norm(&"tree".to_string())
                .blue(&master.config.display())
                .blue(&format!("[{}/{}]",
                               master.totals.processes,
                               master.totals.threads))
                .blue(&format_memory(master.totals.memory))
                .unwrap(),
            children: master.trees.iter()
                .map(|(name, tree)| print_tree(name, tree, opt))
                .collect(),
        });
    }

    let mut out = stdout();
    for tree in trees.iter() {
        try!(tree.print(&mut out));
    }

    return Ok(());
}

fn print_json(scan: ScanResult, _opt: &Options) -> Result<(), IoError> {
    let mut trees = vec!();

    for (_, master) in scan.masters.iter() {
        let mut knots = vec!();
        for (_, instance) in master.trees.iter()
            .flat_map(|(_, tree)| tree.children.iter())
            .flat_map(|(_, child)| child.instances.iter())
        {
            let mut processes = vec!();
            for grp in instance.heads.iter() {
                processes.push(json::Object(vec!(
                    ("pid".to_string(), json::U64(grp.head.pid as u64)),
                    ("processes".to_string(),
                        json::U64(grp.totals.processes as u64)),
                    ("threads".to_string(),
                        json::U64(grp.totals.threads as u64)),
                    ("mem_rss".to_string(),
                        json::U64(grp.totals.mem_rss as u64)),
                    ("mem_swap".to_string(),
                        json::U64(grp.totals.mem_swap as u64)),
                    ("start_time".to_string(),
                        json::U64(start_time_sec(grp.head.start_time))),
                    ("user_time".to_string(),
                        json::U64(grp.totals.user_time)),
                    ("system_time".to_string(),
                        json::U64(grp.totals.system_time)),
                    ("child_user_time".to_string(),
                        json::U64(grp.totals.child_user_time)),
                    ("child_system_time".to_string(),
                        json::U64(grp.totals.child_system_time)),
                    ).into_iter().collect()));
            }
            knots.push(json::Object(vec!(
                ("name".to_string(), json::String(instance.name.to_string())),
                ("pid".to_string(), json::U64(instance.knot_pid as u64)),
                ("ok".to_string(), json::Boolean(instance.heads.len() == 1)),
                ("processes".to_string(), json::List(processes)),
                ).into_iter().collect()));
        }
        trees.push(json::Object(vec!(
            ("pid".to_string(), json::U64(master.pid as u64)),
            ("config".to_string(),
                json::String(master.config.display().to_string())),
            ("children".to_string(),
                json::List(knots)),
            ).into_iter().collect()));
    }


    let mut out = stdout();
    return out.write_str(json::encode(&json::Object(vec!(
        ("trees".to_string(), json::List(trees)),
        ).into_iter().collect())).as_slice());
}

fn monitor_changes(scan: ScanResult, _opt: &Options) -> Result<(), IoError> {
    let mut old_children: TreeMap<String, Instance> = scan.masters.into_iter()
        .flat_map(|(_, master)| master.trees.into_iter())
        .flat_map(|(_, tree)| tree.children.into_iter())
        .flat_map(|(_, child)| child.instances.into_iter())
        .map(|(_, inst)| (inst.name.to_string(), inst))
        .collect();
    let mut old_time = get_time();
    loop {
        sleep(Duration::seconds(1));

        let new_children: TreeMap<String, Instance> = try!(scan_processes())
            .masters.into_iter()
            .flat_map(|(_, master)| master.trees.into_iter())
            .flat_map(|(_, tree)| tree.children.into_iter())
            .flat_map(|(_, child)| child.instances.into_iter())
            .map(|(_, inst)| (inst.name.to_string(), inst))
            .collect();
        let new_time = get_time();
        let delta_ticks = ((new_time - old_time).num_milliseconds() as u64
            * unsafe { clock_ticks }) as f32 / 1000f32;

        let mut pids = vec!();
        let mut names = vec!();
        let mut cpus = vec!();
        let mut mem = vec!();
        let mut threads = vec!();
        let mut processes = vec!();
        for (name, inst) in new_children.iter() {
            if inst.heads.len() == 1 {
                pids.push(inst.heads[0].head.pid as uint);
            } else {
                pids.push(0);
            }
            let ticks = old_children.find(name)
                .and_then(|old|
                    inst.totals.cpu_time.checked_sub(&old.totals.cpu_time))
                .unwrap_or(0);
            names.push(inst.name.to_string());
            cpus.push((ticks as f32/delta_ticks) * 100.);
            threads.push(inst.totals.threads);
            processes.push(inst.totals.processes);
            mem.push(inst.totals.memory);
        }

        print!("\x1b[2J\x1b[;H");
        ascii::render_table(&[
            ("PID", ascii::Ordinal(pids)),
            ("NAME", ascii::Text(names)),
            ("CPU", ascii::Percent(cpus)),
            ("THR", ascii::Ordinal(threads)),
            ("PRC", ascii::Ordinal(processes)),
            ("MEM", ascii::Bytes(mem)),
            ]);

        old_children = new_children;
        old_time = new_time;
    }
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

    read_global_consts();

    let mut action = print_full_tree;
    let mut options = Options {
        printer_factory: ascii::Printer::factory(stdout().get_ref()),
    };

    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut action)
            .add_option(["--json"], box StoreConst(print_json),
                "Print big json instead human-readable tree")
            .add_option(["--monitor"], box StoreConst(monitor_changes),
                "Print big json instead human-readable tree");
        ap.refer(&mut options.printer_factory)
            .add_option(["--force-color"],
                box StoreConst(ascii::Printer::color_factory()),
                "Force colors in output (in default mode only for now)")
            .add_option(["--no-color"],
                box StoreConst(ascii::Printer::plain_factory()),
                "Don't use colors even for terminal output");
        ap.set_description("Displays tree of processes");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match scan_processes().and_then(|s| action(s, &options)) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
