use std::rc::Rc;
use std::comm::{channel, Empty};
use std::from_str::FromStr;
use std::default::Default;
use serialize::Decodable;

use quire::validate::{Validator, Structure, Scalar, Numeric};
use quire;

use super::container_config::ContainerKind;

#[deriving(Decodable, Encodable, PartialEq)]
pub struct ChildConfig {
    pub instances: uint,
    pub image: String,
    pub config: String,
    pub kind: ContainerKind,
}

impl ChildConfig {
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return box Structure { members: vec!(
            ("instances".to_string(), box Numeric {
                default: Some(1u),
                .. Default::default()} as Box<Validator>),
            ("image".to_string(), box Scalar {
                .. Default::default() } as Box<Validator>),
            ("config".to_string(), box Scalar {
                .. Default::default()} as Box<Validator>),
            ("kind".to_string(), box Scalar {
                default: Some("Daemon".to_string()),
                .. Default::default() } as Box<Validator>),
        ), .. Default::default() } as Box<Validator>;
    }
}

impl FromStr for ChildConfig {
    fn from_str(body: &str) -> Option<ChildConfig> {
        quire::parser::parse(
                Rc::new("<command-line>".to_string()),
                body,
                |doc| { quire::ast::process(Default::default(), doc) })
        .ok()
        .and_then(|(ast, _)| {
            let (tx, rx) = channel();
            let mut dec = quire::decode::YamlDecoder::new(ast, tx);
            let res = Decodable::decode(&mut dec);
            assert!(rx.try_recv().unwrap_err() == Empty);
            res.ok()
        })
    }
}
