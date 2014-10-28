use std::rc::Rc;
use std::from_str::FromStr;
use std::default::Default;
use serialize::Decodable;

use quire::validate::{Validator, Structure, Scalar, Numeric};
use quire;

#[deriving(Decodable, Encodable)]
pub struct ChildConfig {
    pub instances: uint,
    pub image: Path,
    pub config: Path,
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
            let mut dec = quire::decode::YamlDecoder::new(ast);
            Decodable::decode(&mut dec).ok()
        })
    }
}
