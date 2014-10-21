use std::rc::Rc;
use std::collections::TreeMap;
use std::collections::HashMap;
use std::collections::PriorityQueue;
use std::mem::swap;
use libc::pid_t;
use time::Timespec;

use super::container::Command;
use super::signal;

pub enum MonitorResult {
    Killed,
    Reboot,
}

pub trait Executor {
    fn command(&self) -> Command;
}

pub struct Process<'a> {
    name: Rc<String>,
    current_pid: Option<pid_t>,
    executor: Box<Executor + 'a>,
}

pub struct Monitor<'a> {
    myname: String,
    processes: TreeMap<Rc<String>, Process<'a>>,
    startqueue: PriorityQueue<(Timespec, Rc<String>)>,
    pids: HashMap<pid_t, Rc<String>>,
    allow_reboot: bool,
}

impl<'a> Monitor<'a> {
    pub fn new<'x>(name: String) -> Monitor<'x> {
        return Monitor {
            myname: name,
            processes: TreeMap::new(),
            pids: HashMap::new(),
            allow_reboot: false,
            startqueue: PriorityQueue::new(),
        };
    }
    pub fn allow_reboot(&mut self) {
        self.allow_reboot = true;
    }
    pub fn add(&mut self, name: Rc<String>, executor: Box<Executor>,
        pid: Option<pid_t>)
    {
        if pid.is_some() {
            info!("[{:s}] Registered process pid: {} as name: {}",
                self.myname, pid, name);
        }
        self.processes.insert(name.clone(), Process {
            name: name,
            current_pid: pid,
            executor: executor});
    }
    pub fn has(&self, name: &Rc<String>) -> bool {
        return self.processes.contains_key(name);
    }
    fn _wait_signal(&self) -> signal::Signal {
        return signal::wait_next(self.allow_reboot);
    }
    pub fn run(&mut self) -> MonitorResult {
        debug!("[{:s}] Starting with {} processes",
            self.myname, self.processes.len());
        for (name, prc) in self.processes.iter_mut() {
            if prc.current_pid.is_some() {
                continue;
            }
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
            let sig = self._wait_signal();
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
                signal::Reboot => {
                    return Reboot;
                }
            }
        }
        info!("[{:s}] Shutting down", self.myname);
        // Shut down loop
        let mut processes = TreeMap::new();
        swap(&mut processes, &mut self.processes);
        let mut left: TreeMap<pid_t, Process> = processes.into_iter()
            .filter(|&(_, ref prc)| prc.current_pid.is_some())
            .map(|(_, prc)| (prc.current_pid.unwrap(), prc))
            .collect();
        while left.len() > 0 {
            let sig = self._wait_signal();
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
                signal::Reboot => {
                    return Reboot;
                }
            }
        }
        return Killed;
    }
}
