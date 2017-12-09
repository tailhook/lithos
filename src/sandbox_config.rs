use std::collections::BTreeMap;
use std::str::FromStr;
use std::path::{PathBuf, Path, Component};

use id_map::{IdMap, mapping_validator};
use range::Range;
use quire::validate::{Sequence, Mapping, Scalar, Numeric};
use quire::validate::{Structure};
use serde::de::{Deserializer, Deserialize, Error};


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
    pub allow_groups: Vec<Range>,
    pub allow_tcp_ports: Vec<Range>,
    pub additional_hosts: BTreeMap<String, String>,
    pub uid_map: Vec<IdMap>,
    pub gid_map: Vec<IdMap>,
    pub auto_clean: bool,
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
        .member("allow_groups", Sequence::new(Scalar::new()))
        .member("allow_tcp_ports", Sequence::new(Scalar::new()))
        .member("uid_map", mapping_validator())
        .member("gid_map", mapping_validator())
        .member("additional_hosts", Mapping::new(
            Scalar::new(),
            Scalar::new()))
        .member("auto_clean", Scalar::new().default("true").optional())
    }
}
