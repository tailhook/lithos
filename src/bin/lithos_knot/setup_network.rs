use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::os::unix::io::AsRawFd;

use blake2::{self, Digest};
use failure::{Error, ResultExt};
use ipnetwork::IpNetwork;
use nix::sched::{setns, CLONE_NEWNET};
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

fn _setup(sandbox: &SandboxConfig, child: &ChildConfig,
    _container: &InstantiatedConfig)
    -> Result<(), Error>
{
    let net = sandbox.bridged_network.as_ref().expect("bridged network");
    let ip = child.ip_addresses.get(0).expect("ip address");
    let (pid, ppid) = get_real_pids()?;
    let my_ns = File::open("/proc/self/ns/net")
        .context("can't open namespace")?;
    let parent_ns = File::open(&format!("/proc/{}/ns/net", ppid))
        .context("can't open parent namespace")?;
    let interface = interface_name(net, ip);
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
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    // jump into parent namespace to add to bridge and up the interface
    setns(parent_ns.as_raw_fd(), CLONE_NEWNET)?;

    let mut cmd = unshare::Command::new("/sbin/brctl");
    cmd.arg("addif").arg(&net.bridge).arg(&interface);
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("brctl failed: {}", s),
        Err(e) => bail!("brctl failed: {}", e),
    }

    let mut cmd = unshare::Command::new("/sbin/ip");
    cmd.arg("link").arg("set");
    cmd.arg(&interface);
    cmd.arg("up");
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => bail!("ip link failed: {}", s),
        Err(e) => bail!("ip link failed: {}", e),
    }

    // and again to the child to setup internal part and routing
    setns(my_ns.as_raw_fd(), CLONE_NEWNET)?;

    let mut cmd = unshare::Command::new("/sbin/ip");
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

    let mut cmd = unshare::Command::new("/sbin/ip");
    cmd.arg("link").arg("set");
    cmd.arg(&iinterface);
    cmd.arg("up");
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
        match cmd.status() {
            Ok(s) if s.success() => {}
            Ok(s) => bail!("ip route failed: {}", s),
            Err(e) => bail!("ip route failed: {}", e),
        }
    }
    Ok(())
}