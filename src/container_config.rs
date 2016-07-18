use std::net::IpAddr;
use std::path::PathBuf;
use std::default::Default;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::os::unix::io::RawFd;

use rustc_serialize::{Decodable, Decoder, Encodable, Encoder};
use quire::validate::{Structure, Sequence, Scalar, Numeric, Enum};
use quire::validate::{Mapping};
use id_map::{IdMap, IdMapExt, mapping_validator};


#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub struct TmpfsInfo {
    pub size: usize,
    pub mode: u32,
}

#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub struct PersistentInfo {
    pub path: PathBuf,
    pub mkdir: bool,
    pub mode: u32,
    pub user: u32,
    pub group: u32,
}

#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub struct StatedirInfo {
    pub path: PathBuf,
    pub mode: u32,
    pub user: u32,
    pub group: u32,
}

#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub enum Volume {
    Readonly(PathBuf),
    Persistent(PersistentInfo),
    Tmpfs(TmpfsInfo),
    Statedir(StatedirInfo),
}

#[derive(RustcDecodable, RustcEncodable, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    Daemon,
    Command,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct ResolvConf {
    pub copy_from_host: bool,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct HostsFile {
    pub copy_from_host: bool,
    pub localhost: Option<bool>,
    pub public_hostname: Option<bool>,
}

pub struct Host(pub IpAddr);

#[derive(RustcDecodable, RustcEncodable)]
pub struct TcpPort {
    pub host: Host,
    pub fd: RawFd,
    pub reuse_addr: bool,
    pub reuse_port: bool,
    pub listen_backlog: usize,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct ContainerConfig {
    pub kind: ContainerKind,
    pub volumes: BTreeMap<String, Volume>,
    pub user_id: u32,
    pub group_id: u32,
    pub restart_timeout: f32,
    pub memory_limit: u64,
    pub fileno_limit: u64,
    pub cpu_shares: usize,
    pub executable: String,
    pub arguments: Vec<String>,
    pub environ: BTreeMap<String, String>,
    pub workdir: PathBuf,
    pub resolv_conf: ResolvConf,
    pub hosts_file: HostsFile,
    pub uid_map: Vec<IdMap>,
    pub gid_map: Vec<IdMap>,
    pub stdout_stderr_file: Option<PathBuf>,
    pub restart_process_only: bool,
    pub tcp_ports: HashMap<u16, TcpPort>,
}


impl ContainerConfig {
    pub fn map_uid(&self, internal_uid: u32) -> Option<u32> {
        self.uid_map.map_id(internal_uid)
    }
    pub fn map_gid(&self, internal_gid: u32) -> Option<u32> {
        self.gid_map.map_id(internal_gid)
    }
    pub fn validator<'x>() -> Structure<'x> {
        Structure::new()
        .member("kind", Scalar::new().default("Daemon"))
        .member("volumes", Mapping::new(
                Scalar::new(),
                volume_validator()))
        .member("user_id", Numeric::new())
        .member("group_id", Numeric::new().default(0))
        .member("memory_limit", Numeric::new().default(0x7fffffffffffffffi64))
        .member("fileno_limit", Numeric::new().default(1024))
        .member("cpu_shares", Numeric::new().default(1024))
        .member("restart_timeout", Numeric::new().min(0).max(86400).default(1))
        .member("executable", Scalar::new())
        .member("arguments", Sequence::new(Scalar::new()))
        .member("environ", Mapping::new(
                Scalar::new(),
                Scalar::new()))
        .member("workdir", Scalar::new().default("/"))
        .member("resolv_conf", Structure::new()
            .member("copy_from_host", Scalar::new().default(true)))
        .member("hosts_file", Structure::new()
            .member("copy_from_host", Scalar::new().default(false))
            .member("localhost", Scalar::new().optional())
            .member("public_hostname", Scalar::new().optional()))
        .member("uid_map", mapping_validator())
        .member("gid_map", mapping_validator())
        .member("stdout_stderr_file", Scalar::new().optional())
        .member("restart_process_only", Scalar::new().default(false))
        .member("tcp_ports", Mapping::new(
            Numeric::new().min(1).max(65535),
            Structure::new()
                .member("host", Scalar::new().default("0.0.0.0"))
                .member("fd", Numeric::new().min(0).optional())
                .member("reuse_addr", Scalar::new().default(true))
                .member("reuse_port", Scalar::new().default(false))
                .member("listen_backlog", Scalar::new().default(128))
            ))
    }
}

pub fn volume_validator<'x>() -> Enum<'x> {
    Enum::new()
    .option("Persistent",  Structure::new()
        .member("path",  Scalar::new().default("/"))
        .member("mkdir",  Scalar::new().default(false))
        .member("mode",  Numeric::new().min(0).max(0o1777).default(0o777))
        .member("user",  Numeric::new().default(0))
        .member("group",  Numeric::new().default(0)))
    .option("Readonly", Scalar::new())
    .option("Tmpfs", Structure::new()
        .member("size", Numeric::new().min(0).default(100*1024*1024))
        .member("mode", Numeric::new().min(0).max(0o1777).default(0o777)))
    .option("Statedir", Structure::new()
        .member("path", Scalar::new().default("/"))
        .member("mode", Numeric::new().min(0).max(0o1777).default(0o777))
        .member("user", Numeric::new().default(0))
        .member("group", Numeric::new().default(0)))
}

impl Decodable for Host {
    fn decode<D: Decoder>(d: &mut D) -> Result<Self, D::Error> {
        try!(d.read_str()).parse().map(Host)
            .map_err(|x| d.error(&format!("{}", x)))
    }
}

impl Encodable for Host {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        format!("{}", self.0).encode(s)
    }
}
