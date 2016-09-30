use std::rc::Rc;
use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError::Empty;
use std::str::FromStr;
use std::default::Default;
use rustc_serialize::Decodable;

use quire::validate::{Structure, Scalar, Numeric, Mapping};
use quire;

use super::container_config::ContainerKind;

#[derive(RustcDecodable, RustcEncodable, PartialEq)]
pub struct ChildConfig {
    pub instances: usize,
    pub image: String,
    pub config: String,
    pub kind: ContainerKind,
}

impl ChildConfig {
    pub fn mapping_validator<'x>() -> Mapping<'x> {
        return Mapping::new(
            Scalar::new(),
            ChildConfig::validator());
    }
    pub fn validator<'x>() -> Structure<'x> {
        Structure::new()
        .member("instances", Numeric::new().default(1))
        .member("image", Scalar::new())
        .member("config", Scalar::new())
        .member("kind", Scalar::new().default("Daemon"))
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
