use std::default::Default;
use std::collections::BTreeMap;

use quire::validate::{Validator, Structure, Sequence, Scalar, Numeric};
use quire::validate::{Mapping};
use self::Volume::*;

//  TODO(tailhook) Currently we parse the string into the following
//  enum, but in future we should just decode into it
pub enum Volume {
    Readonly(Path),
    Persistent(Path),
    Tmpfs(String),
    Statedir(Path),
}

#[derive(Decodable, Encodable, Show, PartialEq, Eq)]
pub enum ContainerKind {
    Daemon,
    Command,
}

#[derive(Decodable, Encodable)]
pub struct ResolvConf {
    pub copy_from_host: bool,
}

#[derive(Decodable, Encodable)]
pub struct HostsFile {
    pub localhost: bool,
    pub public_hostname: bool,
}

#[derive(Decodable, Encodable, Copy)]
pub struct IdMap {
    pub inside: u32,
    pub outside: u32,
    pub count: u32,
}

#[derive(Decodable, Encodable)]
pub struct ContainerConfig {
    pub kind: ContainerKind,
    pub volumes: BTreeMap<String, String>,
    pub user_id: u32,
    pub group_id: u32,
    pub restart_timeout: f32,
    pub memory_limit: u64,
    pub fileno_limit: u64,
    pub cpu_shares: usize,
    pub executable: String,
    pub arguments: Vec<String>,
    pub environ: BTreeMap<String, String>,
    pub workdir: Path,
    pub resolv_conf: ResolvConf,
    pub hosts_file: HostsFile,
    pub uid_map: Vec<IdMap>,
    pub gid_map: Vec<IdMap>,
    pub stdout_stderr_file: Option<Path>,
}

fn mapping_validator<'x>() -> Box<Validator + 'x> {
    return Box::new(Sequence {
        element: Box::new(Structure { members: vec!(
            ("inside".to_string(), Box::new(Numeric {
                default: None::<u32>,
                .. Default::default() }) as Box<Validator>),
            ("outside".to_string(), Box::new(Numeric {
                default: None::<u32>,
                .. Default::default() }) as Box<Validator>),
            ("count".to_string(), Box::new(Numeric {
                default: None::<u32>,
                .. Default::default() }) as Box<Validator>),
            ), .. Default::default() }) as Box<Validator>,
        .. Default::default() }) as Box<Validator>;
}

impl ContainerConfig {
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return Box::new(Structure { members: vec!(
            ("kind".to_string(), Box::new(Scalar {
                default: Some("Daemon".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("volumes".to_string(), Box::new(Mapping {
                key_element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                value_element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
            .. Default::default() }) as Box<Validator>),
            ("user_id".to_string(), Box::new(Numeric {
                default: None::<u32>,
                .. Default::default()}) as Box<Validator>),
            ("group_id".to_string(), Box::new(Numeric {
                default: Some(0u32),
                .. Default::default()}) as Box<Validator>),
            ("memory_limit".to_string(), Box::new(Numeric {
                default: Some(0xffffffffffffffffu64),
                .. Default::default()}) as Box<Validator>),
            ("fileno_limit".to_string(), Box::new(Numeric {
                default: Some(1024u64),
                .. Default::default()}) as Box<Validator>),
            ("cpu_shares".to_string(), Box::new(Numeric {
                default: Some(1024us),
                .. Default::default()}) as Box<Validator>),
            ("restart_timeout".to_string(), Box::new(Numeric {
                min: Some(0.),
                max: Some(86400.),
                default: Some(1f32),
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
        ), .. Default::default() }) as Box<Validator>;
    }
}

pub fn parse_volume(val: &str) -> Result<Volume, String> {
    if val.starts_with("/") {
        let p = Path::new(val);
        if !p.is_absolute() {
            return Err(format!("Volume path must be absolute: \"{}\"",
                               p.display()));
        }
        return Ok(Readonly(p));
    } else if val.starts_with("rw:") {
        let p = Path::new(val.slice_from(3));
        if !p.is_absolute() {
            return Err(format!("Volume path must be absolute: \"{}\"",
                               p.display()));
        }
        return Ok(Persistent(p));
    } else if val.starts_with("state:") {
        let p = Path::new(val.slice_from(6));
        if !p.is_absolute() {
            return Err(format!("Volume path must be absolute: \"{}\"",
                               p.display()));
        }
        return Ok(Statedir(p));
    } else if val.starts_with("tmpfs:") {
        // TODO(tailhook) validate parameters
        return Ok(Tmpfs(val.slice_from(6).to_string()));
    } else {
        return Err(format!("Unknown volume type {}", val));
    }
}
