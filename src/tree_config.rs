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
    pub config_file: Option<PathBuf>,
    pub image_dir: PathBuf,
    pub log_file: Option<PathBuf>,
    pub log_level: Option<String>,
    pub readonly_paths: BTreeMap<PathBuf, PathBuf>,
    pub writable_paths: BTreeMap<PathBuf, PathBuf>,
    pub allow_users: Vec<Range>,
    pub allow_groups: Vec<Range>,
    pub allow_tcp_ports: Vec<Range>,
    pub additional_hosts: BTreeMap<String, String>,
}

impl TreeConfig {
    pub fn validator<'x>() -> Structure<'x> {
        Structure::new()
        .member("config_file", Scalar::new().optional())
        .member("image_dir", Scalar::new().optional()
            .default("/var/lib/lithos/containers"))
        .member("log_file", Scalar::new().optional())
        .member("log_level", Scalar::new().optional())
        .member("readonly_paths", Mapping::new(
            Scalar::new(),
            Scalar::new()))
        .member("writable_paths", Mapping::new(
            Scalar::new(),
            Scalar::new()))
        .member("allow_users", Sequence::new(Scalar::new()))
        .member("allow_groups", Sequence::new(Scalar::new()))
        .member("allow_tcp_ports", Sequence::new(Scalar::new()))
        .member("additional_hosts", Mapping::new(
            Scalar::new(),
            Scalar::new()))
    }
}
