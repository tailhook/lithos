use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::{PathBuf, Path, Component};

use id_map::{IdMap, mapping_validator};
use ipnetwork::IpNetwork;
use quire::validate::{Sequence, Mapping, Scalar, Numeric};
use quire::validate::{Structure};
use range::Range;


#[derive(Deserialize, Clone)]
pub struct BridgedNetwork {
    pub bridge: String,
    #[serde(with="::serde_str")]
    pub network: IpNetwork,
    pub default_gateway: Option<IpAddr>,
    pub after_setup_command: Vec<String>,
}

#[derive(Deserialize)]
pub struct SandboxConfig {
    pub config_file: Option<PathBuf>,
    pub image_dir: PathBuf,
    pub image_dir_levels: u32,
    pub used_images_list: Option<PathBuf>,
    pub log_file: Option<PathBuf>,
    pub log_level: Option<String>,
    pub readonly_paths: BTreeMap<PathBuf, PathBuf>,
    pub writable_paths: BTreeMap<PathBuf, PathBuf>,
    pub allow_users: Vec<Range>,
    pub default_user: Option<u32>,
    pub allow_groups: Vec<Range>,
    pub default_group: Option<u32>,
    pub allow_tcp_ports: Vec<Range>,
    pub additional_hosts: BTreeMap<String, String>,
    pub uid_map: Vec<IdMap>,
    pub gid_map: Vec<IdMap>,
    pub auto_clean: bool,
    pub resolv_conf: PathBuf,
    pub hosts_file: PathBuf,
    pub bridged_network: Option<BridgedNetwork>,
    pub secrets_private_key: Option<PathBuf>,
    pub secrets_namespaces: Vec<String>,
}

impl SandboxConfig {
    pub fn check_path<P: AsRef<Path>>(&self, path: P) -> bool {
        let mut num = 0;
        for component in path.as_ref().components() {
            match component {
                Component::Normal(x) if x.to_str().is_some() => num += 1,
                _ => return false,
            }
        }
        return num == self.image_dir_levels;
    }
    pub fn validator<'x>() -> Structure<'x> {
        Structure::new()
        .member("config_file", Scalar::new().optional())
        .member("image_dir", Scalar::new().optional()
            .default("/var/lib/lithos/containers"))
        .member("image_dir_levels",
            Numeric::new().min(1).max(16).default(1))
        .member("used_images_list", Scalar::new().optional())
        .member("log_file", Scalar::new().optional())
        .member("log_level", Scalar::new().optional())
        .member("readonly_paths", Mapping::new(
            Scalar::new(),
            Scalar::new()))
        .member("writable_paths", Mapping::new(
            Scalar::new(),
            Scalar::new()))
        .member("allow_users", Sequence::new(Scalar::new()))
        .member("default_user", Scalar::new().optional())
        .member("allow_groups", Sequence::new(Scalar::new()))
        .member("default_group", Scalar::new().default(0))
        .member("allow_tcp_ports", Sequence::new(Scalar::new()))
        .member("uid_map", mapping_validator())
        .member("gid_map", mapping_validator())
        .member("additional_hosts", Mapping::new(
            Scalar::new(),
            Scalar::new()))
        .member("auto_clean", Scalar::new().default("true").optional())
        .member("hosts_file", Scalar::new().default("/etc/hosts"))
        .member("resolv_conf", Scalar::new().default("/etc/resolv.conf"))
        .member("bridged_network", Structure::new()
            .member("bridge", Scalar::new())
            .member("network", Scalar::new())
            .member("default_gateway", Scalar::new().optional())
            .member("after_setup_command", Sequence::new(Scalar::new()))
            .optional())
        .member("secrets_private_key", Scalar::new().optional())
        .member("secrets_namespaces", Sequence::new(Scalar::new()))
    }
}
