use std::rc::Rc;
use std::io::BufRead;
use std::fs::{File, create_dir, remove_dir};
use std::io::ErrorKind::NotFound;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::default::Default;
use std::collections::BTreeMap;
use libc::pid_t;
use libc::getpid;


#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct CGroupPath(pub String, pub PathBuf);

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Controller {
    Cpu,
    Memory,
}


#[derive(Default)]
pub struct ParsedCGroups {
    pub all_groups: Vec<Rc<CGroupPath>>,
    pub by_name: BTreeMap<String, Rc<CGroupPath>>,
}

pub struct CGroups {
    full_paths: BTreeMap<Controller, PathBuf>
}


pub fn parse_cgroups(pid: Option<pid_t>) -> Result<ParsedCGroups, String> {
    let path = Path::new(pid.map(|x| format!("/proc/{}/cgroup", x))
                            .unwrap_or("/proc/self/cgroup".to_string()));
    let f = try!(File::open(&path)
                 .map_err(|e| format!("Error reading cgroup: {}", e)));
    let mut f = BufRead::new(f);
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
            if res.by_name.insert(name.to_string(), grp.clone()).is_some() {
                return Err(format!("Duplicate CGroup encountered"));
            }
        }
    }
    return Ok(res);
}

pub fn ensure_in_group(name: &String, controllers: &Vec<String>)
    -> Result<CGroups, String>
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
    debug!("Setting up cgroup {} with controllers {:?}", name, controllers);
    // TODO(tailhook) do we need to customize cgroup mount points?
    let cgroup_base = Path::new("/sys/fs/cgroup");

    let root_path = Path::new("/");

    let parent_grp = try!(parse_cgroups(Some(1)));
    let old_grp = try!(parse_cgroups(None));
    let mypid = unsafe { getpid() };
    let mut res = CGroups { full_paths: BTreeMap::new() };

    for ctr in controllers.iter() {
        let CGroupPath(ref rfolder, ref rpath) = **try!(
            parent_grp.by_name.get(ctr)
            .ok_or(format!("CGroup {} not mounted", ctr)));
        let CGroupPath(ref ofolder, ref opath) = **try!(
            old_grp.by_name.get(ctr)
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
            try!(create_dir(&fullpath)
                 .map_err(|e| format!("Error creating cgroup dir: {}", e)));
        } else {
            debug!("CGroup {} already exists", fullpath.display());
        }
        debug!("Adding task to cgroup {}", fullpath.display());
        try!(OpenOptions::new().append(true).open(&fullpath.join("tasks"))
             .and_then(|mut f| write!(&mut f, "{}", mypid))
             .map_err(|e| format!(
                "Error adding myself (pid: {}) to the group {}: {}",
                mypid, fullpath.display(), e)));
        match ctr.as_slice() {
            "cpu" => {
                res.full_paths.insert(Controller::Cpu, fullpath);
            }
            "memory" => {
                res.full_paths.insert(Controller::Memory, fullpath);
            }
            _ => {}
        };
    }
    return Ok(res);
}

pub fn remove_child_cgroup(child: &str, master: &String,
    controllers: &Vec<String>)
    -> Result<(), String>
{
    // TODO(tailhook) do we need to customize cgroup mount points?
    let cgroup_base = PathBuf::from("/sys/fs/cgroup");
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

    let root_path = PathBuf::from("/");
    let parent_grp = try!(parse_cgroups(Some(1)));

    for ctr in controllers.iter() {
        let CGroupPath(ref folder, ref path) = **parent_grp.by_name.get(ctr)
            .expect("CGroups already checked");
        let fullpath = cgroup_base.join(folder.as_slice())
            .join(path.path_relative_from(&root_path).unwrap())
            .join(master.as_slice()).join(child);
        remove_dir(&fullpath)
            .map_err(|e| if e.kind != NotFound {
                error!("Error removing cgroup {}: {}", fullpath.display(), e)})
            .ok();
    }
    return Ok(());
}

impl CGroups {
    pub fn set_value(&self, ctr: Controller, key: &str, value: &str)
        -> Result<(), String>
    {
        let path = try!(self.full_paths.get(&ctr)
            .ok_or(format!("Controller {:?} is not initialized", ctr)));
        File::create(&path.join(key))
            .and_then(|mut f| f.write_str(value))
            .map_err(|e| format!("Can't write to cgroup path {:?}/{}: {}",
                path, key, e))
    }
}
