use std::rc::Rc;
use std::io::BufferedReader;
use std::io::fs::{File, mkdir, rmdir};
use std::io::fs::PathExtensions;
use std::io::{ALL_PERMISSIONS, Append, Write};
use std::default::Default;
use std::collections::TreeMap;
use libc::pid_t;
use libc::getpid;


#[deriving(PartialEq, Eq, PartialOrd, Ord)]
pub struct CGroupPath(pub String, pub Path);


#[deriving(Default)]
pub struct ParsedCGroups {
    pub all_groups: Vec<Rc<CGroupPath>>,
    pub by_name: TreeMap<String, Rc<CGroupPath>>,
}


pub fn parse_cgroups(pid: Option<pid_t>) -> Result<ParsedCGroups, String> {
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
        let grp = Rc::new(CGroupPath(group_name.clone(), group_path.clone()));
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
    let default_controllers = vec!(
        "name".to_string(),
        "cpu".to_string(),
        "cpuacct".to_string(),
        "memory".to_string(),
        "blkio".to_string(),
        );
    let controllers = if controllers.len() > 0
        { controllers } else { &default_controllers };
    debug!("Setting up cgroup {} with controllers {}", name, controllers);
    // TODO(tailhook) do we need to customize cgroup mount points?
    let cgroup_base = Path::new("/sys/fs/cgroup");

    let root_path = Path::new("/");

    let parent_grp = try!(parse_cgroups(Some(1)));
    let old_grp = try!(parse_cgroups(None));
    let mypid = unsafe { getpid() };

    for ctr in controllers.iter() {
        let CGroupPath(ref rfolder, ref rpath) = **try!(
            parent_grp.by_name.find(ctr)
            .ok_or(format!("CGroup {} not mounted", ctr)));
        let CGroupPath(ref ofolder, ref opath) = **try!(
            old_grp.by_name.find(ctr)
            .ok_or(format!("CGroup {} not mounted", ctr)));
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

pub fn remove_child_cgroup(child: &str, controllers: &Vec<String>)
    -> Result<(), String>
{
    // TODO(tailhook) do we need to customize cgroup mount points?
    let cgroup_base = Path::new("/sys/fs/cgroup");
    let default_controllers = vec!(
        "name".to_string(),
        "cpu".to_string(),
        "cpuacct".to_string(),
        "memory".to_string(),
        "blkio".to_string(),
        );
    let controllers = if controllers.len() > 0
        { controllers } else { &default_controllers };
    debug!("Removing cgroup {}", child);

    let root_path = Path::new("/");
    let my_grp = try!(parse_cgroups(None));

    for ctr in controllers.iter() {
        let CGroupPath(ref folder, ref gpath) = **my_grp.by_name.find(ctr)
            .expect("CGroups already checked");
        let fullpath = cgroup_base.join(folder.as_slice())
            .join(&gpath.path_relative_from(&root_path).unwrap())
            .join(child);
        rmdir(&fullpath)
            .map_err(|e| error!("Error removing cgroup {}: {}",
                                fullpath.display(), e))
            .ok();
    }
    return Ok(());
}
