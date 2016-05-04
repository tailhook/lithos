use std::default::Default;
use std::path::PathBuf;

use rustc_serialize::{Decoder};
use quire::validate::{Structure, Sequence};
use quire::validate::{Scalar};
use super::utils::ensure_dir;

#[derive(RustcDecodable)]
pub struct MasterConfig {
    pub runtime_dir: PathBuf,
    pub sandboxes_dir: PathBuf,
    pub processes_dir: PathBuf,
    pub state_dir: PathBuf,
    pub mount_dir: PathBuf,
    pub devfs_dir: PathBuf,
    pub default_log_dir: PathBuf,
    pub config_log_dir: PathBuf,
    pub stdio_log_dir: PathBuf,
    pub log_file: PathBuf,
    pub syslog_facility: Option<String>,
    pub syslog_app_name: String,
    pub log_level: String,
    pub cgroup_name: Option<String>,
    pub cgroup_controllers: Vec<String>,
}

impl MasterConfig {
    pub fn validator<'x>() -> Structure<'x> {
        Structure::new()
        .member("sandboxes_dir", Scalar::new().default("./sandboxes"))
        .member("processes_dir", Scalar::new().default("./processes"))
        .member("runtime_dir", Scalar::new().default("/run/lithos"))
        .member("state_dir", Scalar::new().default("state"))
        .member("mount_dir", Scalar::new().default("mnt"))
        .member("devfs_dir", Scalar::new()
            .default("/var/lib/lithos/dev"))
        .member("default_log_dir", Scalar::new().default("/var/log/lithos"))
        .member("syslog_facility", Scalar::new().optional())
        .member("syslog_app_name", Scalar::new().default("lithos"))
        .member("log_file", Scalar::new().default("master.log"))
        .member("log_level", Scalar::new().default("warn"))
        .member("config_log_dir", Scalar::new()
            .default("/var/log/lithos/config"))
        .member("stdio_log_dir", Scalar::new()
            .default("/var/log/lithos/stderr"))
        .member("cgroup_name",
            Scalar::new().optional().default("lithos.slice"))
        .member("cgroup_controllers", Sequence::new(Scalar::new()))
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
    try!(ensure_dir(&cfg.stdio_log_dir)
        .map_err(|e| format!("Cant create stdio log dir: {}", e)));
    return Ok(());
}
