use failure::Error;
use std::str::FromStr;
use std::net::IpAddr;
use std::collections::BTreeMap;

use quire::validate::{Structure, Scalar, Numeric, Mapping};
use quire::{Options, parse_string};

#[derive(Serialize, Deserialize)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ChildKind {
    Daemon,
    Command,
}

// Note everything here should be stable-serializable
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ChildConfig {
    pub instances: usize,
    pub image: String,
    pub config: String,
    #[serde(skip_serializing_if="BTreeMap::is_empty", default)]
    pub variables: BTreeMap<String, String>,
    #[serde(skip_serializing_if="Vec::is_empty", default)]
    pub ip_addresses: Vec<IpAddr>,
    pub kind: ChildKind,
}

impl ChildConfig {
    pub fn instantiate(&self, instance: usize) -> Result<ChildConfig, Error>
    {
        let mut cfg = self.clone();
        if self.ip_addresses.len() > 0 {
            if let Some(addr) = self.ip_addresses.get(instance) {
                cfg.ip_addresses = vec![*addr];
            } else {
                bail!("Instance no {}, but there's only {} ip addresses",
                    instance, cfg.ip_addresses.len());
            }
        }
        return Ok(cfg);
    }
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
        .member("variables", Mapping::new(Scalar::new(), Scalar::new()))
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

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use super::ChildConfig;
    use super::ChildKind::Daemon;
    use serde_json::{to_string, from_str};

    #[test]
    fn deserialize_compat() {
        let data = r#"{
            "instances":1,
            "image":"myproj.4a20772b",
            "config":"/config/staging/myproj.yaml",
            "kind":"Daemon"}"#;
        let cc = ChildConfig::from_str(data).unwrap();
        assert_eq!(cc, ChildConfig {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: BTreeMap::new(),
            kind: Daemon,
        });

        let cc: ChildConfig = from_str(&data).unwrap();
        assert_eq!(cc, ChildConfig {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: BTreeMap::new(),
            kind: Daemon,
        });
    }

    #[test]
    fn dedeserialize_vars() {
        let data = r#"{
            "instances":1,
            "image":"myproj.4a20772b",
            "config":"/config/staging/myproj.yaml",
            "variables": {"a": "b"},
            "kind":"Daemon"}"#;
        let cc = ChildConfig::from_str(data).unwrap();
        assert_eq!(cc, ChildConfig {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: vec![
                (String::from("a"), String::from("b")),
            ].into_iter().collect(),
            kind: Daemon,
        })
    }

    #[test]
    fn serialize_compat() {
        let data = to_string(&ChildConfig {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: BTreeMap::new(),
            kind: Daemon,
        }).unwrap();
        assert_eq!(data, "{\
            \"instances\":1,\
            \"image\":\"myproj.4a20772b\",\
            \"config\":\"/config/staging/myproj.yaml\",\
            \"kind\":\"Daemon\"}");
    }

    #[test]
    fn serialize_vars() {
        let data = to_string(&ChildConfig {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: vec![
                (String::from("a"), String::from("b")),
                (String::from("c"), String::from("d")),
            ].into_iter().collect(),
            kind: Daemon,
        }).unwrap();
        assert_eq!(data, "{\
            \"instances\":1,\
            \"image\":\"myproj.4a20772b\",\
            \"config\":\"/config/staging/myproj.yaml\",\
            \"variables\":{\"a\":\"b\",\"c\":\"d\"},\
            \"kind\":\"Daemon\"}");
    }
}
