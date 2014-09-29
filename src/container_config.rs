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
    pub memory_limit: u64,
    pub cpu_shares: uint,
    pub instances: uint,
    pub executable: String,
    pub hostname: String,
    pub arguments: Vec<String>,
    pub environ: TreeMap<String, String>,
}

impl ContainerConfig {
    pub fn validator() -> Box<Validator> {
        return box Structure { members: vec!(
            ("volumes".to_string(), box Mapping {
                key_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                value_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
            } as Box<Validator>),
            ("memory_limit".to_string(), box Numeric {
                default: Some(0xffffffffffffffffu64),
                .. Default::default()} as Box<Validator>),
            ("cpu_shares".to_string(), box Numeric {
                default: Some(1024u),
                .. Default::default()} as Box<Validator>),
            ("instances".to_string(), box Numeric {
                default: Some(1u),
                .. Default::default()} as Box<Validator>),
            ("executable".to_string(), box Scalar {
                .. Default::default() } as Box<Validator>),
            ("hostname".to_string(), box Scalar {
                .. Default::default()} as Box<Validator>),
            ("command".to_string(), box Sequence {
                element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                } as Box<Validator>),
            ("environ".to_string(), box Mapping {
                key_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                value_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
            } as Box<Validator>),
        )} as Box<Validator>;
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
