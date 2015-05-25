use std::default::Default;
use serialize::{Decoder};

use quire::validate::{Validator, Structure, Sequence};
use quire::validate::{Scalar};
use super::utils::ensure_dir;

#[derive(RustcDecodable)]
pub struct MasterConfig {
    pub runtime_dir: Path,
    pub config_dir: Path,
    pub state_dir: Path,
    pub mount_dir: Path,
    pub devfs_dir: Path,
    pub cgroup_name: Option<String>,
    pub cgroup_controllers: Vec<String>,
}

impl MasterConfig {
    pub fn validator<'x>() -> Box<Validator + 'x> {
        return Box::new(Structure { members: vec!(
            ("config_dir".to_string(), Box::new(Scalar {
                default: Some("/etc/lithos".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("runtime_dir".to_string(), Box::new(Scalar {
                default: Some("/run/lithos".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("state_dir".to_string(), Box::new(Scalar {
                default: Some("state".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("mount_dir".to_string(), Box::new(Scalar {
                default: Some("mnt".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("devfs_dir".to_string(), Box::new(Scalar {
                default: Some("/var/lib/lithos/dev".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("cgroup_name".to_string(), Box::new(Scalar {
                optional: true,
                default: Some("lithos.slice".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("cgroup_controllers".to_string(), Box::new(Sequence {
                element: Box::new(Scalar {
                    .. Default::default() }) as Box<Validator>,
                .. Default::default() }) as Box<Validator>),
        ), .. Default::default() }) as Box<Validator>;
    }
}

pub fn create_master_dirs(cfg: &MasterConfig) -> Result<(), String> {
    try!(ensure_dir(&cfg.runtime_dir)
        .map_err(|e| format!("Cant create runtime-dir: {}", e)));
    try!(ensure_dir(&cfg.runtime_dir.join(&cfg.state_dir))
        .map_err(|e| format!("Cant create state-dir: {}", e)));
    try!(ensure_dir(&cfg.runtime_dir.join(&cfg.mount_dir))
        .map_err(|e| format!("Cant create mount-dir: {}", e)));
    return Ok(());
}
