use std::str::FromStr;

use quire::validate::{Structure, Scalar, Numeric, Mapping};
use quire::{Options, parse_string};

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
        parse_string("<command-line>", body,
            &ChildConfig::validator(), &Options::default())
            .map_err(|_| ())
    }
}
