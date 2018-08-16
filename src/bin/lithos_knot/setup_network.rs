use std::fs::File;
use std::io;
use std::net::{IpAddr};
use std::os::unix::io::{AsRawFd, RawFd};
use std::mem::{self, size_of};
use std::process;

use blake2::{self, Digest};
use failure::{Error, ResultExt};
use ipnetwork::IpNetwork;
use libc::{close};
use nix::sched::{setns};
use nix::sched::CloneFlags;
use nix::sys::socket::{SockAddr};
use serde_json::to_vec;
use unshare::{self, Style};

use lithos::child_config::ChildInstance;
use lithos::container_config::{TcpPort, replace_vars};
use lithos::sandbox_config::{BridgedNetwork};


struct NsGuard {
    parent: File,
}

impl NsGuard {
    fn enter(pid: u32) -> Result<NsGuard, Error> {
        let parent_ns = File::open("/proc/self/ns/net")
            .context("can't open parent namespace")?;
        let child_ns = File::open(&format!("/proc/{}/ns/net", pid))
            .context("can't open parent namespace")?;
        setns(child_ns.as_raw_fd(), CloneFlags::CLONE_NEWNET)?;
        Ok(NsGuard {
            parent: parent_ns,
        })
    }
    fn parent_raw_fd(&self) -> RawFd {
        self.parent.as_raw_fd()
    }
}

impl Drop for NsGuard {
    fn drop(&mut self) {
        setns(self.parent.as_raw_fd(), CloneFlags::CLONE_NEWNET)
            .expect("can return into parent namespace");
    }
}


pub fn setup(pid: u32, net: &BridgedNetwork, child: &ChildInstance)
    -> Result<(), String>
{
    if let Some(ip) = child.ip_address {
        _setup_bridged(pid, net, ip)
        .map_err(|e| e.to_string())
    } else {
        _setup_isolated(pid)
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

fn _setup_bridged(pid: u32, net: &BridgedNetwork, ip: IpAddr)
    -> Result<(), Error>
{
    let interface = interface_name(net, &ip);
    let iinterface = interface.replace("_", "-");
    assert!(iinterface != interface);

    {
        // Create interface in the child namespace
        // This helps to keep parent namespace clean if this process crashes
        // for some reason.
        let ns = NsGuard::enter(pid)?;

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
        cmd.arg("netns")
            .arg(&format!("/proc/{}/fd/{}",
                          process::id(), ns.parent_raw_fd()));
        debug!("Running {}", cmd.display(&Style::short()));
        match cmd.status() {
            Ok(s) if s.success() => {}
            Ok(s) => bail!("ip link failed: {}", s),
            Err(e) => bail!("ip link failed: {}", e),
        }
    }  // return into parent namespace to add to bridge and up the interface

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

    {
        // and again to the child to setup internal part and routing
        let _ns = NsGuard::enter(pid)?;

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

        if net.after_setup_command.len() > 0 {
            let mut cmd = unshare::Command::new(&net.after_setup_command[0]);
            for item in &net.after_setup_command[1..] {
                if item.contains('@') {
                    cmd.arg(&replace_vars(item, |v| {
                        match v {
                            "container_ip" => ip.to_string(),
                            _ => {
                                error!("No variable {:?} \
                                        for after-setup-command. \
                                        Using empty string.", v);
                                String::new()
                            }
                        }
                    }));
                } else {
                    cmd.arg(item);
                }
            }
            debug!("Running {}", cmd.display(&Style::short()));
            match cmd.status() {
                Ok(s) if s.success() => {}
                Ok(s) => bail!("after-setup-command failed: {}", s),
                Err(e) => bail!("after-setup-command failed: {}", e),
            }
        }
    }

    Ok(())
}

fn _setup_isolated(child: u32) -> Result<(), Error> {
    let _ns = NsGuard::enter(child)?;
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

/// NOTE!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
/// This function is executed in restricted child environment
///
/// No allocations, logging, etc. Just bare sys calls
pub unsafe fn open_socket(cfg: &TcpPort, addr: &SockAddr)
    -> Result<(), io::Error>
{
    use libc::{socket, setsockopt, bind, listen, dup2, AF_INET, SOCK_STREAM};
    use libc::{SOL_SOCKET, SO_REUSEADDR, SO_REUSEPORT};
    use libc::{fcntl, F_GETFL, F_SETFL, O_NONBLOCK, EINTR};
    let s = match socket(AF_INET, SOCK_STREAM, 0) {
        -1 => return Err(io::Error::last_os_error()),
        s => s,
    };
    // NOTE: we don't close socket on error here, because process will die
    // after any error

    if cfg.reuse_addr {
        if setsockopt(s, SOL_SOCKET, SO_REUSEADDR,
                      mem::transmute(&1u32), size_of::<u32>() as u32) == -1
        {
            return Err(io::Error::last_os_error());
        }
    }
    if cfg.reuse_port {
        if setsockopt(s, SOL_SOCKET, SO_REUSEPORT,
                      mem::transmute(&1u32), size_of::<u32>() as u32) == -1
        {
            return Err(io::Error::last_os_error());
        }
    }
    if cfg.set_non_block {
        let fl = loop {
            let fl = fcntl(s, F_GETFL);
            if fl == -1 {
                let err = io::Error::last_os_error();
                if err.raw_os_error() == Some(EINTR) {
                    continue;
                }
                return Err(err);
            }
            break fl;
        };
        if fcntl(s, F_SETFL, fl | O_NONBLOCK) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    let (sockaddr, len) = addr.as_ffi_pair();
    if bind(s, sockaddr, len) == -1 {
        return Err(io::Error::last_os_error());
    }
    if listen(s, cfg.listen_backlog as i32) == -1 {
        return Err(io::Error::last_os_error());
    }
    if s != cfg.fd {
        if dup2(s, cfg.fd) == -1 {
            return Err(io::Error::last_os_error());
        }
        close(s);
    }
    Ok(())
}
