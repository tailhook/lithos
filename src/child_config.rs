use std::rc::Rc;
use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError::Empty;
use std::str::FromStr;
use std::default::Default;
use serialize::Decodable;

use quire::validate::{Validator, Structure, Scalar, Numeric, Mapping};
use quire;

use super::container_config::ContainerKind;

#[derive(Decodable, Encodable, PartialEq)]
pub struct ChildConfig {
    pub instances: usize,
    pub image: String,
    pub config: String,
    pub kind: ContainerKind,
}

impl ChildConfig {
    pub fn mapping_validator<'x>() -> Box<Validator + 'x> {
        return Box::new(Mapping {
            key_element: Box::new(Scalar {
                .. Default::default()}),
            value_element: ChildConfig::validator(),
            .. Default::default() });
    }
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return Box::new(Structure { members: vec!(
            ("instances".to_string(), Box::new(Numeric {
                default: Some(1us),
                .. Default::default()}) as Box<Validator>),
            ("image".to_string(), Box::new(Scalar {
                .. Default::default() }) as Box<Validator>),
            ("config".to_string(), Box::new(Scalar {
                .. Default::default()}) as Box<Validator>),
            ("kind".to_string(), Box::new(Scalar {
                default: Some("Daemon".to_string()),
                .. Default::default() }) as Box<Validator>),
        ), .. Default::default() }) as Box<Validator>;
    }
}

impl FromStr for ChildConfig {
    type Err = ();
    fn from_str(body: &str) -> Result<ChildConfig, ()> {
        quire::parser::parse(
                Rc::new("<command-line>".to_string()),
                body,
                |doc| { quire::ast::process(Default::default(), doc) })
        .map_err(|_| ())
        .and_then(|(ast, _)| {
            let (tx, rx) = channel();
            let mut dec = quire::decode::YamlDecoder::new(ast, tx);
            let res = Decodable::decode(&mut dec);
            assert!(rx.try_recv().unwrap_err() == Empty);
            res.map_err(|_| ())
        })
    }
}
