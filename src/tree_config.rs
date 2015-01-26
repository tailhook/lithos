use std::default::Default;
use std::collections::TreeMap;
use std::from_str::FromStr;
use serialize::{Decoder, Decodable};

use quire::validate::{Validator, Structure};
use quire::validate::{Sequence, Mapping, Scalar};

#[deriving(Clone, Show)]
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

impl<E, D:Decoder<E>> Decodable<D, E> for Range {
    fn decode(d: &mut D) -> Result<Range, E> {
        match d.read_str() {
            Ok(val) => {
                let num:Option<u32> = FromStr::from_str(val.as_slice());
                match num {
                    Some(num) => return Ok(Range::new(num, num)),
                    None => {}
                }
                match regex!(r"^(\d+)-(\d+)$").captures(val.as_slice()) {
                    Some(caps) => {
                        return Ok(Range::new(
                            from_str(caps.at(1)).unwrap(),
                            from_str(caps.at(2)).unwrap()));
                    }
                    None => unimplemented!(),
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[deriving(Decodable)]
pub struct TreeConfig {
    pub config_dir: Path,
    pub image_dir: Path,
    pub readonly_paths: TreeMap<Path, Path>,
    pub writable_paths: TreeMap<Path, Path>,
    pub allow_users: Vec<Range>,
    pub allow_groups: Vec<Range>,
    pub additional_hosts: TreeMap<String, String>,
}

impl TreeConfig {
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return box Structure { members: vec!(
            ("config_dir".to_string(), box Scalar {
                default: Some("/etc/lithos/current".to_string()),
                .. Default::default() } as Box<Validator>),
            ("image_dir".to_string(), box Scalar {
                default: Some("/var/lib/lithos/containers".to_string()),
                .. Default::default() } as Box<Validator>),
            ("readonly_paths".to_string(), box Mapping {
                key_element: box Scalar { .. Default::default()},
                value_element: box Scalar { .. Default::default()},
                .. Default::default()} as Box<Validator>),
            ("writable_paths".to_string(), box Mapping {
                key_element: box Scalar { .. Default::default()},
                value_element: box Scalar { .. Default::default()},
                .. Default::default() } as Box<Validator>),
            ("allow_users".to_string(), box Sequence {
                element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                .. Default::default() } as Box<Validator>),
            ("allow_groups".to_string(), box Sequence {
                element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                .. Default::default() } as Box<Validator>),
            ("additional_hosts".to_string(), box Mapping {
                key_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                value_element: box Scalar {
                    .. Default::default() } as Box<Validator>,
                .. Default::default() } as Box<Validator>),
        ), .. Default::default() } as Box<Validator>;
    }
}
