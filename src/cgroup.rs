use std::rc::Rc;
use std::io::BufferedReader;
use std::io::fs::{File, mkdir};
use std::io::fs::PathExtensions;
use std::io::{ALL_PERMISSIONS, Append, Write};
use std::default::Default;
use std::collections::TreeMap;
use libc::pid_t;
use libc::getpid;


#[deriving(PartialEq, Eq, PartialOrd, Ord)]
struct CGroupPath(String, Path);


#[deriving(Default)]
#[allow(dead_code)]  // all_groups is not used yet
struct ParsedCGroups {
    all_groups: Vec<Rc<CGroupPath>>,
    by_name: TreeMap<String, Rc<CGroupPath>>,
}


fn parse_cgroups(pid: Option<pid_t>) -> Result<ParsedCGroups, String> {
    let path = Path::new(pid.map(|x| format!("/proc/{}/cgroup", x))
                            .unwrap_or("/proc/self/cgroup".to_string()));
    let f = try!(File::open(&path)
                 .map_err(|e| format!("Error reading cgroup: {}", e)));
    let mut f = BufferedReader::new(f);
    let mut res: ParsedCGroups = Default::default();
    for line in f.lines() {
        let line = try!(line
                       .map_err(|e| format!("Can't read CGroup file: {}", e)));
        // Line is in form of "123:ctr1[,ctr2][=folder]:/group/path"
        let mut chunks = line.as_slice().splitn(2, ':');
        let num = try!(chunks.next()
                       .ok_or(format!("CGroup num expected")));
        let namechunk = try!(chunks.next()
                             .ok_or(format!("CGroup name expected")));
        let mut namepair = namechunk.splitn(2, '=');
        let names = try!(namepair.next()
                         .ok_or(format!("CGroup names expected")));
        let group_name = namepair.next().unwrap_or(names).to_string();
        let group_path = Path::new(try!(chunks.next()
                                   .ok_or(format!("CGroup path expected")))
                                   .trim());
        let grp = Rc::new(CGroupPath(group_name, group_path));
        res.all_groups.push(grp.clone());
        for name in names.split(',') {
            if !res.by_name.insert(name.to_string(), grp.clone()) {
                return Err(format!("Duplicate CGroup encountered"));
            }
        }
    }
    return Ok(res);
}

pub fn ensure_in_group(name: &String, controllers: &Vec<String>)
    -> Result<(), String>
{
    // TODO(tailhook) do we need to customize cgroup mount points?
    let cgroup_base = Path::new("/sys/fs/cgroup");

    let root_path = Path::new("/");
    let root_grp = try!(parse_cgroups(Some(1)));
    let old_grp = try!(parse_cgroups(None));
    let mypid = unsafe { getpid() };

    for ctr in controllers.iter() {
        let CGroupPath(ref rfolder, ref rpath) = **root_grp.by_name.find(ctr)
            .expect("CGroup name already checked");
        let CGroupPath(ref ofolder, ref opath) = **old_grp.by_name.find(ctr)
            .expect("CGroup old name already checked");
        if ofolder != rfolder {
            return Err(format!("Init process has CGroup hierarchy different \
                                from ours, we can't setup CGroups in any \
                                meaningful way in this case"));
        }

        // TODO(tailhook) do we need to customize nested groups?
        // TODO(tailhook) what if we *are* init process?
        let new_path = rpath.join(name.as_slice());

        if new_path == *opath {
            debug!("Already in cgroup {}:{}", ctr, new_path.display());
            continue;
        }
        let fullpath = cgroup_base.join(ofolder.as_slice()).join(
            new_path.path_relative_from(&root_path).unwrap());
        if !fullpath.exists() {
            debug!("Creating cgroup {}", fullpath.display());
            try!(mkdir(&fullpath, ALL_PERMISSIONS)
                 .map_err(|e| format!("Error creating cgroup dir: {}", e)));
        } else {
            debug!("CGroup {} already exists", fullpath.display());
        }
        debug!("Adding task to cgroup {}", fullpath.display());
        try!(File::open_mode(&fullpath.join("tasks"), Append, Write)
             .and_then(|mut f| write!(f, "{}", mypid))
             .map_err(|e| format!(
                "Error adding myself (pid: {}) to the group {}: {}",
                mypid, fullpath.display(), e)));
    }
    return Ok(());
}
