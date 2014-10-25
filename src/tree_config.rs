use std::default::Default;
use std::collections::TreeMap;

use quire::validate::{Validator, Structure, Mapping, Scalar, Numeric};


// Check if Decodable is ok?
#[deriving(Decodable, Encodable)]
pub struct TreeConfig {
    pub config_dir: String,
    pub state_dir: Path,
    pub mount_dir: String,
    //  Update to Path in next rust
    //  in rust0.12 Path has Ord
    pub readonly_paths: TreeMap<String, String>,
    pub writable_paths: TreeMap<String, String>,
    pub devfs_dir: String,
    pub min_port: u16,
    pub max_port: u16,
}

impl TreeConfig {
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return box Structure { members: vec!(
            ("config_dir".to_string(), box Scalar {
                default: Some("/etc/lithos/current".to_string()),
                .. Default::default() } as Box<Validator>),
            ("state_dir".to_string(), box Scalar {
                default: Some("/run/lithos/state".to_string()),
                .. Default::default() } as Box<Validator>),
            ("mount_dir".to_string(), box Scalar {
                default: Some("/run/lithos/mnt".to_string()),
                .. Default::default() } as Box<Validator>),
            ("readonly_paths".to_string(), box Mapping {
                key_element: box Scalar { .. Default::default()},
                value_element: box Scalar { .. Default::default()},
                .. Default::default()} as Box<Validator>),
            ("writable_paths".to_string(), box Mapping {
                key_element: box Scalar { .. Default::default()},
                value_element: box Scalar { .. Default::default()},
                .. Default::default() } as Box<Validator>),
            ("devfs_dir".to_string(), box Scalar {
                default: Some("/var/lib/lithos/dev".to_string()),
                .. Default::default() } as Box<Validator>),
            ("min_port".to_string(), box Numeric {
                default: Some(1024u16),
                .. Default::default() } as Box<Validator>),
            ("max_port".to_string(), box Numeric {
                default: Some(29999u16),
                .. Default::default() } as Box<Validator>),
        ), .. Default::default() } as Box<Validator>;
    }
}
