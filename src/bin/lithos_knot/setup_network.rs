use std::fs::File;
use std::net::IpAddr;
use std::os::unix::io::AsRawFd;

use blake2::{self, Digest};
use failure::{Error, ResultExt};
use ipnetwork::IpNetwork;
use nix::sched::{setns, CLONE_NEWNET};
use nix::unistd::{getppid, getpid};
use serde_json::to_vec;
use unshare;

use lithos::sandbox_config::{SandboxConfig, BridgedNetwork};
use lithos::child_config::ChildConfig;
use lithos::container_config::InstantiatedConfig;


pub fn setup(sandbox: &SandboxConfig, child: &ChildConfig,
    container: &InstantiatedConfig)
    -> Result<(), String>
{
    _setup(sandbox, child, container)
    .map_err(|e| e.to_string())
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
    let name = format!("li_{:06x}_{:02x}{:02x}",
        blake2::Blake2b::digest(&to_vec(&HashSource {
            bridge: &network.bridge,
            ip: ip,
        }).expect("can always serialize")),
        ip1, ip2);
    assert!(name.len() <= 15);
    return name;
}

fn _setup(sandbox: &SandboxConfig, child: &ChildConfig,
    _container: &InstantiatedConfig)
    -> Result<(), Error>
{
    let net = sandbox.bridged_network.as_ref().expect("bridged network");
    let ip = child.ip_addresses.get(0).expect("ip address");
    let ppid = getppid();
    let my_ns = File::open("/proc/self/ns/net")
        .context("can't open namespace")?;
    let parent_ns = File::open(&format!("/proc/{}/ns/net", ppid))
        .context("can't open parent namespace")?;
    setns(parent_ns.as_raw_fd(), CLONE_NEWNET)?;
    let interface = interface_name(net, ip);
    let iinterface = interface.replace("_", "-");
    assert!(iinterface != interface);

    let mut cmd = unshare::Command::new("ip");
    cmd.arg("link").arg("add");
    cmd.arg(&interface);
    cmd.arg("type").arg("veth");
    cmd.arg("peer").arg("name").arg(&iinterface);
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    let mut cmd = unshare::Command::new("ip");
    cmd.arg("link").arg("set");
    cmd.arg("dev").arg(&iinterface);
    // Note: this is a bit of hack bit it's known to work.
    // 1. We opened our namespace first so `/proc/<PID>/ns/net` is associated
    //    with that namespace even if we change namespace
    // 2. Then we set namespace to a parent one
    // 3. We move interface to ours namespace
    // 4. Then switch our namespace back to expected one
    cmd.arg("netns").arg(&format!("{}", getpid()));
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    let mut cmd = unshare::Command::new("brctl");
    cmd.arg("addif").arg(&net.bridge).arg(&interface);
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("brctl failed: {}", s),
        Err(e) => bail!("brctl failed: {}", e),
    }

    let mut cmd = unshare::Command::new("ip");
    cmd.arg("link").arg("set");
    cmd.arg(&interface);
    cmd.arg("up");
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    setns(my_ns.as_raw_fd(), CLONE_NEWNET)?;

    let mut cmd = unshare::Command::new("ip");
    cmd.arg("addr").arg("add");
    cmd.arg(&format!("{}",
        IpNetwork::new(*ip, net.network.prefix())
        .expect("network asways valid")));
    cmd.arg("dev").arg(&iinterface);
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    let mut cmd = unshare::Command::new("ip");
    cmd.arg("link").arg("set");
    cmd.arg(&iinterface);
    cmd.arg("up");
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    if let Some(gw) = net.default_gateway {
        let mut cmd = unshare::Command::new("ip");
        cmd.arg("route").arg("add");
        cmd.arg("default");
        cmd.arg("via").arg(&format!("{}", gw));
        match cmd.status() {
            Ok(s) if s.success() => {}
            Ok(s) => bail!("ip route failed: {}", s),
            Err(e) => bail!("ip route failed: {}", e),
        }
    }
    Ok(())
}
