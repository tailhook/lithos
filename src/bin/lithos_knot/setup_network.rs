use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, SocketAddr};
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd};

use blake2::{self, Digest};
use failure::{Error, ResultExt};
use ipnetwork::IpNetwork;
use libc::{close};
use nix::sched::{setns};
use nix::sched::CloneFlags;
use nix::sys::socket::sockopt::{ReuseAddr, ReusePort};
use nix::sys::socket::{SockAddr, setsockopt, bind, listen};
use nix::sys::socket::{socket, AddressFamily, SockType, SockFlag, InetAddr};
use serde_json::to_vec;
use unshare::{self, Style};

use lithos::child_config::ChildInstance;
use lithos::container_config::{InstantiatedConfig, TcpPort};
use lithos::sandbox_config::{SandboxConfig, BridgedNetwork};
use lithos::utils;


pub fn setup(sandbox: &SandboxConfig, child: &ChildInstance,
    _container: &InstantiatedConfig)
    -> Result<(), String>
{
    if let Some(ip) = child.ip_address {
        _setup_bridged(sandbox, child, ip)
        .map_err(|e| e.to_string())
    } else {
        _setup_isolated(sandbox, child)
        .map_err(|e| e.to_string())
    }
}



fn interface_name(network: &BridgedNetwork, ip: &IpAddr) -> String {
    #[derive(Serialize)]
    struct HashSource<'a> {
        bridge: &'a str,
        ip: &'a IpAddr,
    }
    let (ip1, ip2) = match *ip {
        IpAddr::V4(ip) => (ip.octets()[2], ip.octets()[3]),
        IpAddr::V6(ip) => (ip.octets()[14], ip.octets()[15]),
    };
    let name = format!("li_{:.6}_{:02x}{:02x}",
        // double formatting because of a bug in generic array
        format!("{:06x}", blake2::Blake2b::digest(&to_vec(&HashSource {
            bridge: &network.bridge,
            ip: ip,
        }).expect("can always serialize"))),
        ip1, ip2);
    assert!(name.len() <= 15);
    return name;
}

fn get_real_pids() -> Result<(u32, u32), Error> {
    let mut pid = None::<u32>;
    let mut ppid = None::<u32>;
    let f = BufReader::new(File::open("/proc/self/status")
        .context("can open /proc/self/status")?);
    for line in f.lines() {
        let line = line.context("can open /proc/self/status")?;
        if line.starts_with("Pid:") {
            pid = Some(line[5..].trim().parse::<u32>()
                .context("can parse pid in /proc/self/status")?);
        } else if line.starts_with("PPid:") {
            ppid = Some(line[5..].trim().parse::<u32>()
                .context("can parse pid in /proc/self/status")?);
        } else {
            continue;
        }
        match (pid, ppid) {
            (Some(pid), Some(ppid)) => return Ok((pid, ppid)),
            _ => continue,
        }
    }
    bail!("can't find ppid");
}

fn _setup_bridged(sandbox: &SandboxConfig, _child: &ChildInstance, ip: IpAddr)
    -> Result<(), Error>
{
    let net = sandbox.bridged_network.as_ref().expect("bridged network");
    let (pid, ppid) = get_real_pids()?;
    let my_ns = File::open("/proc/self/ns/net")
        .context("can't open namespace")?;
    let parent_ns = File::open(&format!("/proc/{}/ns/net", ppid))
        .context("can't open parent namespace")?;
    let interface = interface_name(net, &ip);
    let iinterface = interface.replace("_", "-");
    assert!(iinterface != interface);

    // Create interface in the child namespace
    // This helps to keep parent namespace clean if this process crashes for
    // some reason.
    let mut cmd = unshare::Command::new("/bin/ip");
    cmd.arg("link").arg("add");
    cmd.arg(&interface);
    cmd.arg("type").arg("veth");
    cmd.arg("peer").arg("name").arg(&iinterface);
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    // The move just external part of the interface to the parent namespace
    let mut cmd = unshare::Command::new("/sbin/ip");
    cmd.arg("link").arg("set");
    cmd.arg("dev").arg(&interface);
    cmd.arg("netns").arg(&format!("/proc/{}/fd/{}",
        pid, parent_ns.as_raw_fd()));
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    // jump into parent namespace to add to bridge and up the interface
    setns(parent_ns.as_raw_fd(), CloneFlags::CLONE_NEWNET)?;

    let mut cmd = unshare::Command::new("/sbin/brctl");
    cmd.arg("addif").arg(&net.bridge).arg(&interface);
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("brctl failed: {}", s),
        Err(e) => bail!("brctl failed: {}", e),
    }

    let mut cmd = unshare::Command::new("/sbin/ip");
    cmd.arg("link").arg("set");
    cmd.arg(&interface);
    cmd.arg("up");
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    // and again to the child to setup internal part and routing
    setns(my_ns.as_raw_fd(), CloneFlags::CLONE_NEWNET)?;

    let mut cmd = unshare::Command::new("/sbin/ip");
    cmd.arg("link").arg("set");
    cmd.arg("lo").arg("up");
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    let mut cmd = unshare::Command::new("/sbin/ip");
    cmd.arg("addr").arg("add");
    cmd.arg(&format!("{}",
        IpNetwork::new(ip, net.network.prefix())
        .expect("network asways valid")));
    cmd.arg("dev").arg(&iinterface);
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    let mut cmd = unshare::Command::new("/sbin/ip");
    cmd.arg("link").arg("set");
    cmd.arg(&iinterface);
    cmd.arg("up");
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    if let Some(gw) = net.default_gateway {
        let mut cmd = unshare::Command::new("/sbin/ip");
        cmd.arg("route").arg("add");
        cmd.arg("default");
        cmd.arg("via").arg(&format!("{}", gw));
        debug!("Running {}", cmd.display(&Style::short()));
        match cmd.status() {
            Ok(s) if s.success() => {}
            Ok(s) => bail!("ip route failed: {}", s),
            Err(e) => bail!("ip route failed: {}", e),
        }
    }

    let mut cmd = unshare::Command::new("/usr/bin/arping");
    cmd.arg("-U");
    cmd.arg(&format!("{}", ip));
    cmd.arg("-c1");
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("arping failed: {}", s),
        Err(e) => bail!("arping failed: {}", e),
    }

    Ok(())
}

fn _setup_isolated(_sandbox: &SandboxConfig, _child: &ChildInstance)
    -> Result<(), Error>
{
    let mut cmd = unshare::Command::new("/sbin/ip");
    cmd.arg("link").arg("set");
    cmd.arg("lo").arg("up");
    debug!("Running {}", cmd.display(&Style::short()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }
    Ok(())
}

pub fn listen_fds(sandbox: &SandboxConfig, child: &ChildInstance,
    container: &InstantiatedConfig)
    -> Result<HashMap<RawFd, File>, String>
{
    _listen_fds(sandbox, child, container)
        .map_err(|e| e.to_string())
}

fn _listen_fds(sandbox: &SandboxConfig, _child: &ChildInstance,
    container: &InstantiatedConfig)
    -> Result<HashMap<RawFd, File>, Error>
{
    let mut res = HashMap::new();
    if sandbox.bridged_network.is_some() {
        for (&port_no, port) in &container.tcp_ports {
            if !port.external {
                let sock = open_socket(port_no, port,
                    container.user_id.or(sandbox.default_user).unwrap_or(0),
                    container.group_id.or(sandbox.default_group).unwrap_or(0))?;
                res.insert(port.fd, sock);
            }
        }
    }
    return Ok(res);
}

// TODO(tailhook) this is very similar to one in lithos_tree
fn open_socket(port: u16, cfg: &TcpPort, uid: u32, gid: u32)
    -> Result<File, Error>
{
    let addr = InetAddr::from_std(&SocketAddr::new(cfg.host.0, port));
    let sock = {
        let _fsuid_guard = utils::FsUidGuard::set(uid, gid);
        try!(socket(AddressFamily::Inet,
            SockType::Stream, SockFlag::empty(), None)
            .map_err(|e| format_err!("Can't create socket: {:?}", e)))
    };

    let mut result = Ok(());
    if cfg.reuse_addr {
        result = result.and_then(|_| setsockopt(sock, ReuseAddr, &true));
    }
    if cfg.reuse_port {
        result = result.and_then(|_| setsockopt(sock, ReusePort, &true));
    }
    result =  result.and_then(|_| bind(sock, &SockAddr::Inet(addr)));
    result =  result.and_then(|_| listen(sock, cfg.listen_backlog));
    if let Err(e) = result {
        unsafe { close(sock) };
        Err(format_err!("Socket option error: {:?}", e))
    } else {
        Ok(unsafe { File::from_raw_fd(sock) })
    }
}
