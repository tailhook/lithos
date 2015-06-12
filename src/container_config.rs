use std::path::PathBuf;
use std::default::Default;
use std::collections::BTreeMap;

use quire::validate::{Validator, Structure, Sequence, Scalar, Numeric, Enum};
use quire::validate::{Mapping};
use self::Volume::*;


#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub struct TmpfsInfo {
    pub size: usize,
    pub mode: u32,
}

#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub struct PersistentInfo {
    pub path: PathBuf,
    pub mkdir: bool,
    pub mode: u32,
    pub user: u32,
    pub group: u32,
}

#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub struct StatedirInfo {
    pub path: PathBuf,
    pub mode: u32,
    pub user: u32,
    pub group: u32,
}

#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub enum Volume {
    Readonly(PathBuf),
    Persistent(PersistentInfo),
    Tmpfs(TmpfsInfo),
    Statedir(StatedirInfo),
}

#[derive(RustcDecodable, RustcEncodable, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    Daemon,
    Command,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct ResolvConf {
    pub copy_from_host: bool,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct HostsFile {
    pub localhost: bool,
    pub public_hostname: bool,
}

#[derive(RustcDecodable, RustcEncodable, Clone, Copy)]
pub struct IdMap {
    pub inside: u32,
    pub outside: u32,
    pub count: u32,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct ContainerConfig {
    pub kind: ContainerKind,
    pub volumes: BTreeMap<String, Volume>,
    pub user_id: u32,
    pub group_id: u32,
    pub restart_timeout: f32,
    pub memory_limit: u64,
    pub fileno_limit: u64,
    pub cpu_shares: usize,
    pub executable: String,
    pub arguments: Vec<String>,
    pub environ: BTreeMap<String, String>,
    pub workdir: PathBuf,
    pub resolv_conf: ResolvConf,
    pub hosts_file: HostsFile,
    pub uid_map: Vec<IdMap>,
    pub gid_map: Vec<IdMap>,
    pub stdout_stderr_file: Option<PathBuf>,
    pub restart_process_only: bool,
}

fn mapping_validator<'x>() -> Box<Validator + 'x> {
    return Box::new(Sequence {
        element: Box::new(Structure { members: vec!(
            ("inside".to_string(), Box::new(Numeric {
                default: None,
                .. Default::default() }) as Box<Validator>),
            ("outside".to_string(), Box::new(Numeric {
                default: None,
                .. Default::default() }) as Box<Validator>),
            ("count".to_string(), Box::new(Numeric {
                default: None,
                .. Default::default() }) as Box<Validator>),
            ), .. Default::default() }) as Box<Validator>,
        .. Default::default() }) as Box<Validator>;
}

impl ContainerConfig {
    pub fn map_uid(&self, uid: u32) -> Option<u32> {
        _map_id(&self.uid_map, uid)
    }
    pub fn map_gid(&self, gid: u32) -> Option<u32> {
        _map_id(&self.gid_map, gid)
    }
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return Box::new(Structure { members: vec!(
            ("kind".to_string(), Box::new(Scalar {
                default: Some("Daemon".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("volumes".to_string(), Box::new(Mapping {
                key_element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                value_element: volume_validator(),
            .. Default::default() }) as Box<Validator>),
            ("user_id".to_string(), Box::new(Numeric {
                default: None,
                .. Default::default()}) as Box<Validator>),
            ("group_id".to_string(), Box::new(Numeric {
                default: Some(0),
                .. Default::default()}) as Box<Validator>),
            ("memory_limit".to_string(), Box::new(Numeric {
                default: Some(0xffffffffffffffffi64),
                .. Default::default()}) as Box<Validator>),
            ("fileno_limit".to_string(), Box::new(Numeric {
                default: Some(1024),
                .. Default::default()}) as Box<Validator>),
            ("cpu_shares".to_string(), Box::new(Numeric {
                default: Some(1024),
                .. Default::default()}) as Box<Validator>),
            ("restart_timeout".to_string(), Box::new(Numeric {
                min: Some(0),
                max: Some(86400),
                default: Some(1),
                .. Default::default()}) as Box<Validator>),
            ("executable".to_string(), Box::new(Scalar {
                .. Default::default() }) as Box<Validator>),
            ("arguments".to_string(), Box::new(Sequence {
                element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                .. Default::default() }) as Box<Validator>),
            ("environ".to_string(), Box::new(Mapping {
                key_element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                value_element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
            .. Default::default() }) as Box<Validator>),
            ("workdir".to_string(), Box::new(Scalar {
                default: Some("/".to_string()),
                .. Default::default()}) as Box<Validator>),
            ("resolv_conf".to_string(), Box::new(Structure { members: vec!(
                ("copy_from_host".to_string(), Box::new(Scalar {
                    default: Some("true".to_string()),
                    .. Default::default() }) as Box<Validator>),
                ), .. Default::default()}) as Box<Validator>),
            ("hosts_file".to_string(), Box::new(Structure { members: vec!(
                ("localhost".to_string(), Box::new(Scalar {
                    default: Some("true".to_string()),
                    .. Default::default() }) as Box<Validator>),
                ("public_hostname".to_string(), Box::new(Scalar {
                    default: Some("true".to_string()),
                    .. Default::default() }) as Box<Validator>),
                ), .. Default::default()}) as Box<Validator>),
            ("uid_map".to_string(), mapping_validator()),
            ("gid_map".to_string(), mapping_validator()),
            ("stdout_stderr_file".to_string(), Box::new(Scalar {
                optional: true,
                default: None,
                .. Default::default() }) as Box<Validator>),
            ("restart_process_only".to_string(), Box::new(Scalar {
                default: Some("false".to_string()),
                .. Default::default() }) as Box<Validator>),
        ), .. Default::default() }) as Box<Validator>;
    }
}

pub fn volume_validator<'a>() -> Box<Validator + 'a> {
    return Box::new(Enum { options: vec!(
        ("Persistent".to_string(),  Box::new(Structure { members: vec!(
            ("path".to_string(),  Box::new(Scalar {
                default: Some("/".to_string()),
                .. Default::default()}) as Box<Validator>),
            ("mkdir".to_string(),  Box::new(Scalar {
                default: Some("false".to_string()),
                .. Default::default()}) as Box<Validator>),
            ("mode".to_string(),  Box::new(Numeric {
                min: Some(0),
                max: Some(0o700),
                default: Some(0o766),
                .. Default::default()}) as Box<Validator>),
            ("user".to_string(),  Box::new(Numeric {
                default: Some(0),
                .. Default::default()}) as Box<Validator>),
            ("group".to_string(),  Box::new(Numeric {
                default: Some(0),
                .. Default::default()}) as Box<Validator>),
            ),.. Default::default()}) as Box<Validator>),
        ("Readonly".to_string(), Box::new(Scalar {
            .. Default::default()}) as Box<Validator>),
        ("Tmpfs".to_string(),  Box::new(Structure { members: vec!(
            ("size".to_string(),  Box::new(Numeric {
                min: Some(0),
                default: Some(100*1024*1024),
                .. Default::default()}) as Box<Validator>),
            ("mode".to_string(),  Box::new(Numeric {
                min: Some(0),
                max: Some(0o1777),
                default: Some(0o766),
                .. Default::default()}) as Box<Validator>),
            ),.. Default::default()}) as Box<Validator>),
        ("Statedir".to_string(),  Box::new(Structure { members: vec!(
            ("path".to_string(),  Box::new(Scalar {
                default: Some("/".to_string()),
                .. Default::default()}) as Box<Validator>),
            ("mode".to_string(),  Box::new(Numeric {
                min: Some(0),
                max: Some(0o700),
                default: Some(0o766),
                .. Default::default()}) as Box<Validator>),
            ("user".to_string(),  Box::new(Numeric {
                default: Some(0),
                .. Default::default()}) as Box<Validator>),
            ("group".to_string(),  Box::new(Numeric {
                default: Some(0),
                .. Default::default()}) as Box<Validator>),
            ),.. Default::default()}) as Box<Validator>),
        ), .. Default::default()}) as Box<Validator>;
}

fn _map_id(map: &Vec<IdMap>, id: u32) -> Option<u32> {
    if map.len() == 0 {
        return Some(id);
    }
    for rng in map.iter() {
        if id >= rng.outside && id <= rng.outside + rng.count {
            return Some(rng.inside + (id - rng.outside));
        }
    }
    None
}
