use std::io;
use std::io::{Write, stderr};
use std::path::{Path};
use std::time::SystemTime;

use log;
use fern;
use syslog;
use humantime::format_rfc3339_seconds;

use super::master_config::MasterConfig;
use super::utils::{clean_dir};
use super::cgroup;



pub fn clean_child(name: &str, master: &MasterConfig, temporary: bool) {
    let st_dir = master.runtime_dir
        .join(&master.state_dir).join(name);
    clean_dir(&st_dir, true)
        .map_err(|e| error!("Error removing state dir for {}: {}", name, e))
        .ok();
    if !temporary {
        // If shutdown is temporary (i.e. process failed and we are going to
        // restart it shortly), we don't remove cgroups. Because removing
        // them triggers the following bug in the memory cgroup controller:
        //
        // https://lkml.org/lkml/2016/6/15/1135
        //
        // I mean this is still not fixed in linux 4.6, so while we may be
        // able to get rid of this. But this won't gonna happen in 2-3 years :(
        //
        // Anyway it's possible that we don't need this in the new (unified)
        // cgroup hierarhy which is already there in 4.5, but we don't support
        // it yet.
        if let Some(ref master_grp) = master.cgroup_name {
            let cgname = name.replace("/", ":") + ".scope";
            cgroup::remove_child_cgroup(&cgname, master_grp,
                                        &master.cgroup_controllers)
                .map_err(|e| error!("Error removing cgroup: {}", e))
                .ok();
        }
    }
}

pub fn init_logging(cfg: &MasterConfig, suffix: &Path, name: &str,
    log_stderr: bool, level: log::LogLevel)
    -> Result<(), String>
{
    let sysfac = cfg.syslog_facility.as_ref()
        .and_then(|v| v.parse()
            .map_err(|_| writeln!(&mut stderr(),
                "Can't parse syslog facility: {:?}. Syslog is disabled.", v))
            .ok());
    if let Some(facility) = sysfac {
        syslog::init(facility, level.to_log_level_filter(), Some(&name))
        .map_err(|e| format!("Can't initialize logging: {}", e))
    } else {
        let path = cfg.default_log_dir.join(suffix);
        let file = fern::log_file(path)
            .map_err(|e| format!("Can't initialize logging: {}", e))?;
        let mut disp = fern::Dispatch::new()
            .format(|out, message, record| {
                if record.level() >= log::LogLevel::Debug {
                    out.finish(format_args!("[{}][{}]{}:{}: {}",
                        format_rfc3339_seconds(SystemTime::now()),
                        record.level(),
                        record.location().file(), record.location().line(),
                        message))
                } else {
                    out.finish(format_args!("[{}][{}] {}",
                        format_rfc3339_seconds(SystemTime::now()),
                        record.level(), message))
                }
            })
            .level(level.to_log_level_filter())
            .chain(file);
        if log_stderr {
            disp = disp.chain(io::stderr())
        }
        disp.apply()
            .map_err(|e| format!("Can't initialize logging: {}", e))
    }
}
