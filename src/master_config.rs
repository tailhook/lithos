use std::default::Default;
use std::path::PathBuf;

use rustc_serialize::{Decoder};
use quire::validate::{Validator, Structure, Sequence};
use quire::validate::{Scalar};
use super::utils::ensure_dir;

#[derive(RustcDecodable)]
pub struct MasterConfig {
    pub runtime_dir: PathBuf,
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub mount_dir: PathBuf,
    pub devfs_dir: PathBuf,
    pub default_log_dir: PathBuf,
    pub config_log_dir: PathBuf,
    pub log_file: PathBuf,
    pub log_level: String,
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
            ("default_log_dir".to_string(), Box::new(Scalar {
                default: Some("/var/log/lithos".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("log_file".to_string(), Box::new(Scalar {
                default: Some("master.log".to_string()),
                .. Default::default() }) as Box<Validator>),
            ("log_level".to_string(), Box::new(Scalar {
                default: Some(String::from("warn")),
                .. Default::default() }) as Box<Validator>),
            ("config_log_dir".to_string(), Box::new(Scalar {
                default: Some("/var/log/lithos/config".to_string()),
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
    try!(ensure_dir(&cfg.default_log_dir)
        .map_err(|e| format!("Cant create log dir: {}", e)));
    try!(ensure_dir(&cfg.config_log_dir)
        .map_err(|e| format!("Cant create configuration log dir: {}", e)));
    return Ok(());
}
