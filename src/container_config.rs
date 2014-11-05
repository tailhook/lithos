use std::default::Default;
use std::collections::TreeMap;

use quire::validate::{Validator, Structure, Sequence, Scalar, Numeric};
use quire::validate::{Mapping};

//  TODO(tailhook) Currently we parse the string into the following
//  enum, but in future we should just decode into it
pub enum Volume {
    Readonly(Path),
    Persistent(Path),
    Tmpfs(String),
    Statedir(Path),
}

#[deriving(Decodable, Encodable)]
pub struct ResolvConf {
    pub copy_from_host: bool,
}

#[deriving(Decodable, Encodable)]
pub struct HostsFile {
    pub localhost: bool,
    pub public_hostname: bool,
}

#[deriving(Decodable, Encodable)]
pub struct ContainerConfig {
    pub volumes: TreeMap<String, String>,
    pub user_id: uint,
    pub restart_timeout: f32,
    pub memory_limit: u64,
    pub cpu_shares: uint,
    pub executable: String,
    pub arguments: Vec<String>,
    pub environ: TreeMap<String, String>,
    pub workdir: Path,
    pub resolv_conf: ResolvConf,
    pub hosts_file: HostsFile,
}

impl ContainerConfig {
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return box Structure { members: vec!(
            ("volumes".to_string(), box Mapping {
                key_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                value_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
            .. Default::default() } as Box<Validator>),
            ("user_id".to_string(), box Numeric {
                min: Some(1),
                max: Some(65534),
                default: None::<uint>,
                .. Default::default()} as Box<Validator>),
            ("memory_limit".to_string(), box Numeric {
                default: Some(0xffffffffffffffffu64),
                .. Default::default()} as Box<Validator>),
            ("cpu_shares".to_string(), box Numeric {
                default: Some(1024u),
                .. Default::default()} as Box<Validator>),
            ("restart_timeout".to_string(), box Numeric {
                min: Some(0.),
                max: Some(86400.),
                default: Some(1f32),
                .. Default::default()} as Box<Validator>),
            ("executable".to_string(), box Scalar {
                .. Default::default() } as Box<Validator>),
            ("command".to_string(), box Sequence {
                element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                .. Default::default() } as Box<Validator>),
            ("environ".to_string(), box Mapping {
                key_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                value_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
            .. Default::default() } as Box<Validator>),
            ("workdir".to_string(), box Scalar {
                default: Some("/".to_string()),
                .. Default::default()} as Box<Validator>),
            ("resolv_conf".to_string(), box Structure { members: vec!(
                ("copy_from_host".to_string(), box Scalar {
                    default: Some("true".to_string()),
                    .. Default::default() } as Box<Validator>),
                ), .. Default::default()} as Box<Validator>),
            ("hosts_file".to_string(), box Structure { members: vec!(
                ("localhost".to_string(), box Scalar {
                    default: Some("true".to_string()),
                    .. Default::default() } as Box<Validator>),
                ("public_hostname".to_string(), box Scalar {
                    default: Some("true".to_string()),
                    .. Default::default() } as Box<Validator>),
                ), .. Default::default()} as Box<Validator>),
        ), .. Default::default() } as Box<Validator>;
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
