use std::collections::TreeMap;
use libc::pid_t;

use super::container::Command;
use super::signal;

pub trait Executor {
    fn command(&self) -> Command;
}

pub struct Process<'a> {
    current_pid: Option<pid_t>,
    executor: Box<Executor>,
}

pub struct Monitor<'a> {
    myname: String,
    processes: TreeMap<String, Process<'a>>,
}

impl<'a> Monitor<'a> {
    pub fn new(name: String) -> Monitor {
        return Monitor {
            myname: name,
            processes: TreeMap::new(),
        };
    }
    pub fn add(&mut self, name: String, executor: Box<Executor>) {
        self.processes.insert(name, Process {
            current_pid: None,
            executor: executor});
    }
    pub fn run(&mut self) {
        debug!("Starting with {} processes", self.processes.len());
        for (name, prc) in self.processes.mut_iter() {
            match prc.executor.command().spawn() {
                Ok(pid) => {
                    info!("[{:s}] Process {} started with pid {}",
                        self.myname, name, pid);
                    prc.current_pid = Some(pid);
                }
                Err(e) => {
                    error!("Can't run container {}: {}", name, e);
                    // TODO(tailhook) add to restart-later list
                }
            }
        }
        loop {
            let sig = signal::wait_next();
            info!("[{:s}] Got signal {}", self.myname, sig);
        }
    }
}
