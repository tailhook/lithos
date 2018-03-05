use failure::Error;
use std::str::FromStr;
use std::net::IpAddr;
use std::collections::BTreeMap;

use quire::validate::{Structure, Scalar, Numeric, Mapping, Sequence};
use quire::{Options, parse_string};

#[derive(Serialize, Deserialize)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ChildKind {
    Daemon,
    Command,
}

// Note everything here should be stable-serializable
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ChildInstance {
    pub instances: usize,  // legacy maybe remove somehow?
    pub image: String,
    pub config: String,
    #[serde(skip_serializing_if="BTreeMap::is_empty", default)]
    pub variables: BTreeMap<String, String>,
    #[serde(skip_serializing_if="Option::is_none", default)]
    pub ip_address: Option<IpAddr>,
    pub kind: ChildKind,
}

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
    pub fn instantiate(&self, instance: usize) -> Result<ChildInstance, Error>
    {
        let cfg = ChildInstance {
            instances: 1,  // TODO(tailhook) legacy, find a way to remove
            image: self.image.clone(),
            config: self.config.clone(),
            variables: self.variables.clone(),
            ip_address: if self.ip_addresses.len() > 0 {
                if let Some(addr) = self.ip_addresses.get(instance) {
                    Some(*addr)
                } else {
                    bail!("Instance no {}, but there's only {} ip addresses",
                        instance, self.ip_addresses.len());
                }
            } else {
                None
            },
            kind: self.kind,
        };
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
        .member("ip_addresses", Sequence::new(Scalar::new()))
    }
}
impl ChildInstance {
    pub fn validator<'x>() -> Structure<'x> {
        Structure::new()
        .member("instances", Numeric::new().default(1))
        .member("image", Scalar::new())
        .member("config", Scalar::new())
        .member("variables", Mapping::new(Scalar::new(), Scalar::new()))
        .member("kind", Scalar::new().default("Daemon"))
        .member("ip_address", Scalar::new().optional())
    }
}

impl FromStr for ChildInstance {
    type Err = ();
    fn from_str(body: &str) -> Result<ChildInstance, ()> {
        parse_string("<command-line>", body,
            &Self::validator(), &Options::default())
            .map_err(|_| ())
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use super::ChildInstance;
    use super::ChildKind::Daemon;
    use serde_json::{to_string, from_str};

    #[test]
    fn deserialize_compat() {
        let data = r#"{
            "instances":1,
            "image":"myproj.4a20772b",
            "config":"/config/staging/myproj.yaml",
            "kind":"Daemon"}"#;
        let cc = ChildInstance::from_str(data).unwrap();
        assert_eq!(cc, ChildInstance {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: BTreeMap::new(),
            ip_address: None,
            kind: Daemon,
        });

        let cc: ChildInstance = from_str(&data).unwrap();
        assert_eq!(cc, ChildInstance {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: BTreeMap::new(),
            ip_address: None,
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
        let cc = ChildInstance::from_str(data).unwrap();
        assert_eq!(cc, ChildInstance {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: vec![
                (String::from("a"), String::from("b")),
            ].into_iter().collect(),
            ip_address: None,
            kind: Daemon,
        })
    }

    #[test]
    fn serialize_compat() {
        let data = to_string(&ChildInstance {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: BTreeMap::new(),
            ip_address: None,
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
        let data = to_string(&ChildInstance {
            instances: 1,
            image: String::from("myproj.4a20772b"),
            config: String::from("/config/staging/myproj.yaml"),
            variables: vec![
                (String::from("a"), String::from("b")),
                (String::from("c"), String::from("d")),
            ].into_iter().collect(),
            ip_address: None,
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
