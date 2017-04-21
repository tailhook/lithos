use std::net::IpAddr;
use std::path::PathBuf;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::os::unix::io::RawFd;

use regex::{Regex, Captures};
use rustc_serialize::{Decodable, Decoder, Encodable, Encoder};
use quire::validate::{Structure, Sequence, Scalar, Numeric, Enum};
use quire::validate::{Mapping, Nothing};
use id_map::{IdMap, IdMapExt, mapping_validator};

use sandbox_config::SandboxConfig;
use utils::{in_range};


pub const DEFAULT_KILL_TIMEOUT: f32 = 5.;

lazy_static! {
    static ref VARIABLE_REGEX: Regex = Regex::new(r#"@\{[^}]*\}"#).unwrap();
}


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

#[derive(RustcDecodable, RustcEncodable, Serialize, Debug, PartialEq, Eq)]
#[derive(Clone, Copy)]
pub enum ContainerKind {
    Daemon,
    Command,
}

#[derive(RustcDecodable, RustcEncodable, Clone)]
pub struct ResolvConf {
    pub copy_from_host: bool,
}

#[derive(RustcDecodable, RustcEncodable, Clone)]
pub struct HostsFile {
    pub copy_from_host: bool,
    pub localhost: Option<bool>,
    pub public_hostname: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct Host(pub IpAddr);

#[derive(RustcDecodable, RustcEncodable, Clone)]
pub struct TcpPort {
    pub host: Host,
    pub fd: RawFd,
    pub reuse_addr: bool,
    pub reuse_port: bool,
    pub listen_backlog: usize,
}

#[derive(RustcDecodable, RustcEncodable, Clone, PartialEq, Eq)]
pub enum Variable {
    TcpPort,
}

#[derive(RustcDecodable)]
pub struct ContainerConfig {
    pub kind: ContainerKind,
    pub variables: BTreeMap<String, Variable>,
    pub volumes: BTreeMap<String, Volume>,
    pub user_id: u32,
    pub group_id: u32,
    pub restart_timeout: f32,
    pub kill_timeout: f32,
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
    pub interactive: bool,
    pub restart_process_only: bool,
    pub tcp_ports: HashMap<String, TcpPort>,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct InstantiatedConfig {
    pub kind: ContainerKind,
    pub volumes: BTreeMap<String, Volume>,
    pub user_id: u32,
    pub group_id: u32,
    pub restart_timeout: f32,
    pub kill_timeout: f32,
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
    pub interactive: bool,
    pub restart_process_only: bool,
    pub tcp_ports: HashMap<u16, TcpPort>,
}


pub struct Variables<'a> {
    pub user_vars: &'a HashMap<String, String>,
    pub lithos_name: &'a str,
    pub lithos_config_filename: &'a str,
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
        .member("variables", Enum::new()
            .option("TcpPort", Nothing)
        )
        .member("volumes", Mapping::new(
                Scalar::new(),
                volume_validator()))
        .member("user_id", Numeric::new())
        .member("group_id", Numeric::new().default(0))
        .member("memory_limit", Numeric::new().default(0x7fffffffffffffffi64))
        .member("fileno_limit", Numeric::new().default(1024))
        .member("cpu_shares", Numeric::new().default(1024))
        .member("restart_timeout", Numeric::new().min(0).max(86400).default(1))
        .member("kill_timeout",
            Numeric::new().min(0).max(86400)
                .default(DEFAULT_KILL_TIMEOUT as i64))
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
        .member("interactive", Scalar::new().default(false))
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
    pub fn instantiate(&self, variables: &Variables)
        -> Result<InstantiatedConfig, Vec<String>>
    {
        let mut errors1 = HashSet::new();
        let mut errors2 = HashSet::new();
        let result = {
            let mut replacer = |capt: &Captures| {
                let varname = capt.get(0).unwrap().as_str();
                let val = variables.user_vars.get(varname).map(|x| x.clone())
                    .or_else(|| match varname {
                        "lithos:name"
                        => Some(variables.lithos_name.to_string()),
                        "lithos:config_filename"
                        => Some(variables.lithos_config_filename.to_string()),
                        _ => None,
                    });
                match val {
                    Some(x) => x,
                    None => {
                        errors1.insert(format!("unknown variable {:?}", varname));
                        return format!("<<no var {:?}>>", varname);
                    }
                }
            };
            InstantiatedConfig {
                kind: self.kind.clone(),
                volumes: self.volumes.clone(),
                user_id: self.user_id.clone(),
                group_id: self.group_id.clone(),
                restart_timeout: self.restart_timeout.clone(),
                kill_timeout: self.kill_timeout.clone(),
                memory_limit: self.memory_limit.clone(),
                fileno_limit: self.fileno_limit.clone(),
                cpu_shares: self.cpu_shares.clone(),
                executable: self.executable.clone(),
                arguments: self.arguments.iter()
                    .map(|x| VARIABLE_REGEX.replace(&x, &mut replacer).into())
                    .collect(),
                environ: self.environ.iter()
                    .map(|(key, val)| {
                        (key.clone(),
                         VARIABLE_REGEX.replace(&val, &mut replacer).into())
                    })
                    .collect(),
                workdir: self.workdir.clone(),
                resolv_conf: self.resolv_conf.clone(),
                hosts_file: self.hosts_file.clone(),
                uid_map: self.uid_map.clone(),
                gid_map: self.gid_map.clone(),
                stdout_stderr_file: self.stdout_stderr_file.clone(),
                interactive: self.interactive.clone(),
                restart_process_only: self.restart_process_only.clone(),
                tcp_ports: self.tcp_ports.iter()
                    .map(|(key, val)| {
                        let s = VARIABLE_REGEX.replace(&key, &mut replacer);
                        let port = match s.parse::<u16>() {
                            Ok(x) => x,
                            Err(e) => {
                                errors2.insert(format!("Bad port {:?}: {}",
                                    key, e));
                                return (0, val.clone());
                            }
                        };
                        (port, val.clone())
                    })
                    .collect(),
            }
        };
        if errors1.len() > 0 || errors2.len() > 0 {
            return Err(errors1.into_iter().chain(errors2.into_iter())
                       .collect());
        } else {
            return Ok(result);
        }
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

impl Variable {
    pub fn validate(&self, value: &str, sandbox: &SandboxConfig)
        -> Result<(), String>
    {
        let port = value.parse::<u16>()
            .map_err(|e| format!("invalid TcpPort {:?}: {}", value, e))?;
        if !in_range(&sandbox.allow_tcp_ports, port as u32) {
            return Err(format!("TcpPort {:?} is not in allowed range", port));
        }
        Ok(())
    }
}
