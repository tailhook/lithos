use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
#[cfg(not(target_arch="wasm32"))] use std::os::unix::io::RawFd;

use serde::de::{Deserializer, Deserialize, Error as DeError};
use serde::ser::{Serializer, Serialize};
use serde_json::Value as Json;
use quire::validate::{Structure, Sequence, Scalar, Numeric, Enum};
use quire::validate::{Mapping, Nothing, Anything};
use id_map::{IdMap, IdMapExt, mapping_validator};

use sandbox_config::SandboxConfig;
use range::{in_range};
use child_config::ChildKind;


pub const DEFAULT_KILL_TIMEOUT: f32 = 5.;

#[cfg(target_arch="wasm32")] type RawFd = i32;

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct TmpfsInfo {
    pub size: usize,
    pub mode: u32,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct PersistentInfo {
    pub path: PathBuf,
    pub mkdir: bool,
    pub mode: u32,
    pub user: u32,
    pub group: u32,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct StatedirInfo {
    pub path: PathBuf,
    pub mode: u32,
    pub user: u32,
    pub group: u32,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum Volume {
    Readonly(PathBuf),
    Persistent(PersistentInfo),
    Tmpfs(TmpfsInfo),
    Statedir(StatedirInfo),
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum ContainerKind {
    Daemon,
    Command,
    CommandOrDaemon,
}

impl ContainerKind {
    pub fn matches(self, child_kind: ChildKind) -> bool {
        use container_config::ContainerKind as L;
        use child_config::ChildKind as R;
        match (self, child_kind) {
            (L::Command, R::Command) => true,
            (L::Daemon, R::Daemon) => true,
            (L::CommandOrDaemon, R::Command) => true,
            (L::CommandOrDaemon, R::Daemon) => true,
            (L::Command, R::Daemon) => false,
            (L::Daemon, R::Command) => false,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ResolvConf {
    pub mount: Option<bool>,
    pub copy_from_host: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct HostsFile {
    pub mount: Option<bool>,
    pub copy_from_host: bool,
    pub localhost: Option<bool>,
    pub public_hostname: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct Host(pub IpAddr);

#[derive(Deserialize, Serialize, Clone)]
pub struct TcpPort {
    pub host: Host,
    pub fd: RawFd,
    pub reuse_addr: bool,
    pub reuse_port: bool,
    pub listen_backlog: usize,
    pub external: bool,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug)]
pub enum Variable {
    TcpPort(TcpPortSettings),
    Name,
    DottedName,
    Choice(Vec<String>),
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug)]
pub struct TcpPortSettings {
    pub activation: Activation,
}

#[derive(Deserialize, Serialize)]
pub struct ContainerConfig {
    pub kind: ContainerKind,
    pub variables: BTreeMap<String, Variable>,
    pub metadata: Json,
    pub volumes: BTreeMap<String, Volume>,
    pub user_id: Option<u32>,
    pub group_id: Option<u32>,
    pub restart_timeout: f32,
    pub kill_timeout: f32,
    pub memory_limit: u64,
    pub fileno_limit: u64,
    pub cpu_shares: usize,
    pub executable: String,
    pub arguments: Vec<String>,
    pub environ: BTreeMap<String, String>,
    pub secret_environ: BTreeMap<String, Vec<String>>,
    pub workdir: PathBuf,
    pub resolv_conf: ResolvConf,
    pub hosts_file: HostsFile,
    pub uid_map: Vec<IdMap>,
    pub gid_map: Vec<IdMap>,
    pub stdout_stderr_file: Option<PathBuf>,
    pub interactive: bool,
    pub restart_process_only: bool,
    pub normal_exit_codes: BTreeSet<i32>,
    pub tcp_ports: HashMap<String, TcpPort>,
}

#[derive(Deserialize, Serialize)]
pub struct InstantiatedConfig {
    pub kind: ContainerKind,
    pub volumes: BTreeMap<String, Volume>,
    pub user_id: Option<u32>,
    pub group_id: Option<u32>,
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
    pub normal_exit_codes: BTreeSet<i32>,
    pub tcp_ports: HashMap<u16, TcpPort>,
    pub pid_env_vars: HashSet<String>,
}


pub struct Variables<'a> {
    pub user_vars: &'a BTreeMap<String, String>,
    pub lithos_name: &'a str,
    pub lithos_config_filename: &'a str,
}

impl InstantiatedConfig {
    pub fn map_uid(&self, internal_uid: u32) -> Option<u32> {
        self.uid_map.map_id(internal_uid)
    }
    pub fn map_gid(&self, internal_gid: u32) -> Option<u32> {
        self.gid_map.map_id(internal_gid)
    }
}

fn wrap_into_list(ast: ::quire::ast::Ast) -> Vec<::quire::ast::Ast> {
    use quire::ast::Ast::Scalar;
    use quire::ast::Tag::NonSpecific;
    use quire::ast::ScalarKind::Plain;
    match ast {
        Scalar(pos, _, _style, value) => {
            vec![Scalar(pos.clone(), NonSpecific, Plain, value)]
        }
        _ => unreachable!(),
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all="kebab-case")]
pub enum Activation {
    Systemd,
    None,
}

impl ContainerConfig {
    pub fn validator<'x>() -> Structure<'x> {
        Structure::new()
        .member("kind", Scalar::new().default("Daemon"))
        .member("variables", Mapping::new(
            Scalar::new(),
            Enum::new()
                .option("TcpPort", Structure::new()
                    .member("activation", Enum::new()
                        .option("systemd", Nothing)
                        .allow_plain()
                        .plain_default("none")))
                .option("Name", Nothing)
                .option("DottedName", Nothing)
                .option("Choice", Sequence::new(Scalar::new()))
        ))
        .member("metadata", Anything)
        .member("volumes", Mapping::new(
                Scalar::new(),
                volume_validator()))
        .member("user_id", Numeric::new().optional())
        .member("group_id", Numeric::new().optional())
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
        .member("secret_environ", Mapping::new(
                Scalar::new(),
                Sequence::new(Scalar::new())
                .parser(wrap_into_list)))
        .member("workdir", Scalar::new().default("/"))
        .member("resolv_conf", Structure::new()
            .member("mount", Scalar::new().optional())
            .member("copy_from_host", Scalar::new().default(true)))
        .member("hosts_file", Structure::new()
            .member("mount", Scalar::new().optional())
            .member("copy_from_host", Scalar::new().default(true))
            .member("localhost", Scalar::new().optional())
            .member("public_hostname", Scalar::new().optional()))
        .member("uid_map", mapping_validator())
        .member("gid_map", mapping_validator())
        .member("stdout_stderr_file", Scalar::new().optional())
        .member("interactive", Scalar::new().default(false))
        .member("restart_process_only", Scalar::new().default(false))
        .member("normal_exit_codes", Sequence::new(Numeric::new()))
        .member("tcp_ports", Mapping::new(
            Scalar::new(),
            Structure::new()
                .member("host", Scalar::new().default("0.0.0.0"))
                .member("fd", Numeric::new().min(0).optional())
                .member("reuse_addr", Scalar::new().default(true))
                .member("reuse_port", Scalar::new().default(false))
                .member("listen_backlog", Scalar::new().default(128))
                .member("external", Scalar::new().default(false))
            ))
    }
    pub fn instantiate(&self, variables: &Variables)
        -> Result<InstantiatedConfig, Vec<String>>
    {
        let mut errors1 = HashSet::new();
        let mut errors2 = HashSet::new();
        let mut errors3 = Vec::new();
        let result = {
            let mut replacer = |varname: &str| {
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
                        if varname == "lithos:pid" {
                            errors1.insert("lithos:pid variable \
                                can only be used in environment as a sole \
                                value".into());
                        } else {
                            errors1.insert(format!("unknown variable {:?}",
                                varname));
                        }
                        return format!("<<no var {:?}>>", varname);
                    }
                }
            };
            let mut tcp_ports = self.tcp_ports.iter()
                .map(|(key, val)| {
                    let s = replace_vars(&key, &mut replacer);
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
                .collect::<HashMap<_, _>>();

            let mut pid_env_vars = HashSet::new();
            let mut environ = self.environ.iter()
                .map(|(key, val)| {
                    if val == "${lithos:pid}" {
                        pid_env_vars.insert(key.clone());
                        (key.clone(), "".into())
                    } else {
                        (key.clone(), replace_vars(&val, &mut replacer).into())
                    }
                })
                .collect::<BTreeMap<_, _>>();

            let mut names = Vec::new();
            for (key, typ) in &self.variables {
                match typ {
                    Variable::TcpPort(TcpPortSettings {
                        activation: Activation::Systemd
                    }) => {
                        names.push(&key[..]);
                        let fd = (2 + names.len()) as i32;
                        let port_str = match variables.user_vars.get(key) {
                            None => {
                                errors3.push(
                                    format_err!("can't find var {:?}", key));
                                continue;
                            }
                            Some(port_str) => port_str,
                        };
                        let port = match port_str.parse() {
                            Err(e) => {
                                errors3.push(format_err!("can't parse port \
                                    {:?}: value {:?}: {}", key, port_str, e));
                                continue;
                            }
                            Ok(port) => port,
                        };
                        tcp_ports.insert(port, TcpPort {
                            host: Host(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
                            fd,
                            reuse_addr: true,
                            reuse_port: false,
                            listen_backlog: 128,
                            external: false,
                        });
                    }
                    _ => {}
                }
            }
            if !names.is_empty() {
                environ.insert("LISTEN_FDS".into(), names.len().to_string());
                environ.insert("LISTEN_FDNAMES".into(), names.join(":"));
                pid_env_vars.insert("LISTEN_PID".into());
            }

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
                    .map(|x| replace_vars(&x, &mut replacer).into())
                    .collect(),
                environ,
                // ignore secret environ, it will be pushed into environ later
                workdir: self.workdir.clone(),
                resolv_conf: self.resolv_conf.clone(),
                hosts_file: self.hosts_file.clone(),
                uid_map: self.uid_map.clone(),
                gid_map: self.gid_map.clone(),
                stdout_stderr_file: self.stdout_stderr_file.clone(),
                interactive: self.interactive.clone(),
                restart_process_only: self.restart_process_only.clone(),
                normal_exit_codes: self.normal_exit_codes.clone(),
                tcp_ports,
                pid_env_vars,
            }
        };
        if errors1.len() > 0 || errors2.len() > 0 || errors3.len() > 0 {
            return Err(errors1.into_iter()
                .chain(errors2.into_iter())
                .chain(errors3.into_iter().map(|x| x.to_string()))
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

impl<'a> Deserialize<'a> for Host {
    fn deserialize<D: Deserializer<'a>>(d: D) -> Result<Host, D::Error> {
        String::deserialize(d)?.parse().map(Host)
            .map_err(|x| D::Error::custom(format!("{}", x)))
    }
}

impl Serialize for Host {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        format!("{}", self.0).serialize(s)
    }
}

impl Variable {
    pub fn validate(&self, value: &str, sandbox: &SandboxConfig)
        -> Result<(), String>
    {
        match *self {
            Variable::TcpPort { .. } => {
                let port = value.parse::<u16>()
                    .map_err(|e| format!(
                        "invalid TcpPort {:?}: {}", value, e))?;
                // TODO(tailhook) This still has an issue with
                //                validating "external" ports.
                //                But we don't know if port is external here.
                if sandbox.bridged_network.is_none() {
                    if !in_range(&sandbox.allow_tcp_ports, port as u32) {
                        return Err(format!(
                            "TcpPort {:?} is not in allowed range", port));
                    }
                }
            }
            Variable::Name => {
                let chars_ok = value.chars().all(|x| {
                    x.is_ascii() && x.is_alphanumeric() || x == '-' || x == '_'
                });
                if !chars_ok {
                    return Err(format!("Value {:?} contains characters that \
                        are invalid for names (alphanumeric, `-` and `_`)",
                        value));
                }
            }
            Variable::DottedName => {
                let chars_ok = value.chars().all(|x| {
                    x.is_ascii() && x.is_alphanumeric() ||
                    x == '-' || x == '_' || x == '.'
                });
                if !chars_ok {
                    return Err(format!("Value {:?} contains characters that \
                        are invalid for dns names (alphanumeric, `-` and `_`)",
                        value));
                }
                for slice in value.split('.') {
                    if slice == "" || slice.starts_with("-") ||
                                      slice.ends_with("-")
                    {
                        return Err(format!("Component {:?} from name {:?} \
                            is invalid for dotted name",
                            slice, value));
                    }
                }
            }
            Variable::Choice(ref choices) => {
                if !choices.iter().any(|x| x == value) {
                    return Err(format!("variable value {:?} \
                        is not one of {:?}", value, choices));
                }
            }
        }
        Ok(())
    }
}

pub fn replace_vars<F, S>(mut s: &str, mut f: F)
    -> String
    where F: FnMut(&str) -> S,
          S: AsRef<str>,
{
    let mut result = String::with_capacity(s.len());
    while let Some(vpos) = s.find("@{") {
        result.push_str(&s[..vpos]);
        s = &s[vpos..];
        if let Some(vend) = s.find('}') {
            let var = s[2..vend].trim();
            result.push_str(f(var).as_ref());
            s = &s[vend+1..];
        } else {
            break;  // unclosed vars are just raw text
        }
    }
    result.push_str(s);
    return result;
}

#[cfg(test)]
mod test {
    use super::replace_vars;

    #[test]
    fn just_var() {
        assert_eq!(replace_vars("@{x}", |_| "1"), "1");
    }

    #[test]
    fn suffix() {
        assert_eq!(replace_vars("xxx@{x}", |_| "1"), "xxx1");
    }

    #[test]
    fn prefix() {
        assert_eq!(replace_vars("@{yy}zzz", |_| "1"), "1zzz");
    }

    #[test]
    fn middle() {
        assert_eq!(replace_vars("aaa@{yy}zzz", |_| "1"), "aaa1zzz");
    }
    #[test]
    fn two_vars() {
        assert_eq!(replace_vars("one @{x} two @{ y } three", |_| "1"),
            "one 1 two 1 three");
    }

    #[test]
    fn correct_name() {
        assert_eq!(replace_vars("@{x}", |name| {
            assert_eq!(name, "x");
            "1"
        }), "1");
        assert_eq!(replace_vars("@{xyz}", |name| {
            assert_eq!(name, "xyz");
            "1"
        }), "1");
        assert_eq!(replace_vars("a@{xyz}b@{xyz}c", |name| {
            assert_eq!(name, "xyz");
            "1"
        }), "a1b1c");
    }
}
