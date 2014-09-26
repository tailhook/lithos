use std::default::Default;
use std::collections::TreeMap;

use quire::validate::{Validator, Structure, Sequence, Scalar, Numeric};
use quire::validate::{Mapping};

/*
TODO(tailhook) use the following volume
enum Volume {
    Readonly(Path),
    Persistent(Path),
    Tmpfs(String),
}
*/

type Volume = String;

#[deriving(Decodable, Encodable)]
pub struct ContainerConfig {
    pub volumes: TreeMap<String, Volume>,
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
