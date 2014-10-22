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
}

#[deriving(Decodable, Encodable)]
pub struct ContainerConfig {
    pub volumes: TreeMap<String, String>,
    pub user_id: uint,
    pub restart_timeout: f32,
    pub memory_limit: u64,
    pub cpu_shares: uint,
    pub instances: uint,
    pub executable: String,
    pub hostname: Option<String>,
    pub arguments: Vec<String>,
    pub environ: TreeMap<String, String>,
    pub workdir: Path,
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
            ("instances".to_string(), box Numeric {
                default: Some(1u),
                .. Default::default()} as Box<Validator>),
            ("executable".to_string(), box Scalar {
                .. Default::default() } as Box<Validator>),
            ("hostname".to_string(), box Scalar {
                optional: true,
                .. Default::default()} as Box<Validator>),
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
    } else if val.starts_with("tmpfs:") {
        // TODO(tailhook) validate parameters
        return Ok(Tmpfs(val.slice_from(6).to_string()));
    } else {
        return Err(format!("Unknown volume type {}", val));
    }
}
