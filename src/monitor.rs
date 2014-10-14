use std::collections::TreeMap;
use std::collections::HashMap;
use std::mem::swap;
use libc::pid_t;

use super::container::Command;
use super::signal;

pub trait Executor {
    fn command(&self) -> Command;
}

pub struct Process<'a> {
    name: String,
    current_pid: Option<pid_t>,
    executor: Box<Executor>,
}

pub struct Monitor<'a> {
    myname: String,
    processes: TreeMap<String, Process<'a>>,
    pids: HashMap<pid_t, String>,
}

impl<'a> Monitor<'a> {
    pub fn new(name: String) -> Monitor {
        return Monitor {
            myname: name,
            processes: TreeMap::new(),
            pids: HashMap::new(),
        };
    }
    pub fn add(&mut self, name: String, executor: Box<Executor>) {
        self.processes.insert(name.clone(), Process {
            name: name,
            current_pid: None,
            executor: executor});
    }
    pub fn has(&self, name: &String) -> bool {
        return self.processes.contains_key(name);
    }
    pub fn run(&mut self) {
        debug!("[{:s}] Starting with {} processes",
            self.myname, self.processes.len());
        for (name, prc) in self.processes.mut_iter() {
            match prc.executor.command().spawn() {
                Ok(pid) => {
                    info!("[{:s}] Process {} started with pid {}",
                        self.myname, name, pid);
                    prc.current_pid = Some(pid);
                    self.pids.insert(pid, name.clone());
                }
                Err(e) => {
                    error!("Can't run container {}: {}", name, e);
                    // TODO(tailhook) add to restart-later list
                }
            }
        }
        // Main loop
        loop {
            let sig = signal::wait_next();
            info!("[{:s}] Got signal {}", self.myname, sig);
            match sig {
                signal::Terminate(sig) => {
                    for (_name, prc) in self.processes.iter() {
                        match prc.current_pid {
                            Some(pid) => signal::send_signal(pid, sig),
                            None => {}
                        }
                    }
                    break;
                }
                signal::Child(pid, status) => {
                    let prc = match self.pids.find(&pid) {
                        Some(name) => self.processes.find_mut(name).unwrap(),
                        None => {
                            warn!("[{:s}] Unknown process {} dead with {}",
                                self.myname, pid, status);
                            continue;
                        },
                    };
                    warn!("[{:s}] Child {}:{} exited with status {}",
                        self.myname, prc.name, pid, status);
                    match prc.executor.command().spawn() {
                        Ok(pid) => {
                            info!("[{:s}] Process {} started with pid {}",
                                self.myname, prc.name, pid);
                            prc.current_pid = Some(pid);
                            self.pids.insert(pid, prc.name.clone());
                        }
                        Err(e) => {
                            error!("Can't run container {}: {}", prc.name, e);
                            // TODO(tailhook) add to restart-later list
                        }
                    }

                }
            }
        }
        info!("[{:s}] Shutting down", self.myname);
        // Shut down loop
        let mut processes = TreeMap::new();
        swap(&mut processes, &mut self.processes);
        let mut left: TreeMap<pid_t, Process> = FromIterator::from_iter(
            processes.move_iter()
            .filter(|&(_, ref prc)| prc.current_pid.is_some())
            .map(|(_, prc)| (prc.current_pid.unwrap(), prc)));
        while left.len() > 0 {
            let sig = signal::wait_next();
            info!("[{:s}] Got signal {}", self.myname, sig);
            match sig {
                signal::Terminate(sig) => {
                    for (_name, prc) in left.iter() {
                        match prc.current_pid {
                            Some(pid) => signal::send_signal(pid, sig),
                            None => {}
                        }
                    }
                }
                signal::Child(pid, status) => {
                    match left.pop(&pid) {
                        Some(prc) => {
                            info!("[{:s}] Child {}:{} exited with status {}",
                                self.myname, prc.name, pid, status);
                        }
                        None => {
                            warn!("[{:s}] Unknown process {} dead with {}",
                                self.myname, pid, status);
                        }
                    }
                }
            }
        }
    }
}
