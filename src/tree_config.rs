use std::default::Default;
use std::collections::BTreeMap;
use std::str::FromStr;
use std::path::PathBuf;
use rustc_serialize::{Decoder, Decodable};
use regex::Regex;

use quire::validate::{Validator, Structure};
use quire::validate::{Sequence, Mapping, Scalar};

#[derive(Clone, Debug)]
pub struct Range {
    pub start: u32,
    pub end: u32,
}

impl Range {
    pub fn new(start: u32, end: u32) -> Range {
        return Range { start: start, end: end };
    }
    pub fn len(&self) -> u32 {
        return self.end - self.start + 1;
    }
    pub fn shift(&self, val: u32) -> Range {
        assert!(self.end - self.start + 1 >= val);
        return Range::new(self.start + val, self.end);
    }
}

impl Decodable for Range {
    fn decode<D:Decoder>(d: &mut D) -> Result<Range, D::Error> {
        match d.read_str() {
            Ok(val) => {
                let num:Result<u32, _> = FromStr::from_str(&val[..]);
                match num {
                    Ok(num) => return Ok(Range::new(num, num)),
                    Err(_) => {}
                }
                let regex = Regex::new(r"^(\d+)-(\d+)$").unwrap();
                match regex.captures(&val[..]) {
                    Some(caps) => {
                        return Ok(Range::new(
                            caps.at(1).and_then(
                                |x| FromStr::from_str(x).ok()).unwrap(),
                            caps.at(2).and_then(
                                |x| FromStr::from_str(x).ok()).unwrap()));
                    }
                    None => unimplemented!(),
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[derive(RustcDecodable)]
pub struct TreeConfig {
    pub config_file: PathBuf,
    pub image_dir: PathBuf,
    pub readonly_paths: BTreeMap<PathBuf, PathBuf>,
    pub writable_paths: BTreeMap<PathBuf, PathBuf>,
    pub allow_users: Vec<Range>,
    pub allow_groups: Vec<Range>,
    pub additional_hosts: BTreeMap<String, String>,
}

impl TreeConfig {
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return Box::new(Structure { members: vec!(
            ("config_file".to_string(), Box::new(Scalar {
                default: Some("/etc/lithos/current.yaml".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("image_dir".to_string(), Box::new(Scalar {
                default: Some("/var/lib/lithos/containers".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("readonly_paths".to_string(), Box::new(Mapping {
                key_element: Box::new(Scalar { .. Default::default()}),
                value_element: Box::new(Scalar { .. Default::default()}),
                .. Default::default()}) as Box<Validator>),
            ("writable_paths".to_string(), Box::new(Mapping {
                key_element: Box::new(Scalar { .. Default::default()}),
                value_element: Box::new(Scalar { .. Default::default()}),
                .. Default::default() }) as Box<Validator>),
            ("allow_users".to_string(), Box::new(Sequence {
                element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                .. Default::default() }) as Box<Validator>),
            ("allow_groups".to_string(), Box::new(Sequence {
                element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                .. Default::default() }) as Box<Validator>),
            ("additional_hosts".to_string(), Box::new(Mapping {
                key_element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                value_element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                .. Default::default() }) as Box<Validator>),
        ), .. Default::default() }) as Box<Validator>;
    }
}
