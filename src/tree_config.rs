use std::default::Default;
use std::collections::TreeMap;

use quire::validate::{Validator, Structure, Mapping, Scalar, Numeric};


#[deriving(Decodable)]
pub struct TreeConfig {
    pub config_dir: Path,
    pub state_dir: Path,
    pub readonly_paths: TreeMap<String, Path>,
    pub writable_paths: TreeMap<String, Path>,
    pub devfs_dir: Path,
    pub min_port: u16,
    pub max_port: u16,
}

impl TreeConfig {
    pub fn validator() -> Box<Validator> {
        return box Structure { members: vec!(
            ("config_dir".to_string(), box Scalar {
                default: Some("/etc/lithos/current".to_string()),
                .. Default::default() } as Box<Validator>),
            ("state_dir".to_string(), box Scalar {
                default: Some("/run/lithos/state".to_string()),
                .. Default::default() } as Box<Validator>),
            ("readonly_paths".to_string(), box Mapping {
                key_element: box Scalar { .. Default::default()},
                value_element: box Scalar { .. Default::default()},
                } as Box<Validator>),
            ("writable_paths".to_string(), box Mapping {
                key_element: box Scalar { .. Default::default()},
                value_element: box Scalar { .. Default::default()},
                } as Box<Validator>),
            ("devfs_dir".to_string(), box Scalar {
                default: Some("/var/lib/lithos/dev".to_string()),
                .. Default::default() } as Box<Validator>),
            ("min_port".to_string(), box Numeric {
                default: Some(1024u16),
                .. Default::default() } as Box<Validator>),
            ("max_port".to_string(), box Numeric {
                default: Some(29999u16),
                .. Default::default() } as Box<Validator>),
        )} as Box<Validator>;
    }
}
