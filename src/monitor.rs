use std::rc::Rc;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::BinaryHeap;
use std::mem::swap;
use libc::pid_t;

use super::container::Command;
use super::signal::Signal;
use super::signal;
use super::utils::{get_time, Time};
use self::PrepareResult::*;

pub enum MonitorResult {
    Killed,
    Reboot,
}

pub enum PrepareResult {
    Run,
    Error(String),
    Shutdown,
}

pub trait Executor {
    fn prepare(&self) -> PrepareResult { return Run; }
    fn command(&self) -> Command;
    fn finish(&self) -> bool { return true; }
}

pub struct Process<'a> {
    name: Rc<String>,
    current_pid: Option<pid_t>,
    start_time: Option<Time>,
    restart_timeout: i64,
    executor: Box<Executor + 'a>,
}

pub struct Monitor<'a> {
    myname: String,
    processes: BTreeMap<Rc<String>, Process<'a>>,
    start_queue: BinaryHeap<(i64, Rc<String>)>,
    pids: HashMap<pid_t, Rc<String>>,
    allow_reboot: bool,
}

fn _top_time(pq: &BinaryHeap<(i64, Rc<String>)>) -> Option<f64> {
    return pq.peek().map(|&(ts, _)| -ts as f64/1000.0);
}

impl<'a> Monitor<'a> {
    pub fn new<'x>(name: String) -> Monitor<'x> {
        return Monitor {
            myname: name,
            processes: BTreeMap::new(),
            pids: HashMap::new(),
            allow_reboot: false,
            start_queue: BinaryHeap::new(),
        };
    }
    pub fn allow_reboot(&mut self) {
        self.allow_reboot = true;
    }
    pub fn add(&mut self, name: Rc<String>, executor: Box<Executor + 'a>,
        timeout: i64, current: Option<(pid_t, Time)>)
    {
        if let Some((pid, _)) = current {
            info!("[{}] Registered process pid: {} as name: {}",
                self.myname, pid, name);
            self.pids.insert(pid, name.clone());
        } else {
            self.start_queue.push((0, name.clone()));
        }
        self.processes.insert(name.clone(), Process::<'a> {
            name: name,
            current_pid: current.map(|(pid, _)| pid),
            start_time: current.map(|(_, time)| time),
            restart_timeout: timeout,
            executor: executor});
    }
    pub fn has(&self, name: &Rc<String>) -> bool {
        return self.processes.contains_key(name);
    }
    fn _wait_signal(&self) -> Signal {
        return signal::wait_next(
            self.allow_reboot,
            _top_time(&self.start_queue));
    }
    fn _start_process(&mut self, name: &Rc<String>) -> PrepareResult {
        let prc = self.processes.get_mut(name).unwrap();
        let prepare_result = prc.executor.prepare();
        match prepare_result {
            Run => {
                match prc.executor.command().spawn() {
                    Ok(pid) => {
                        info!("[{}] Process {} started with pid {}",
                            self.myname, prc.name, pid);
                        prc.current_pid = Some(pid);
                        prc.start_time = Some(get_time());
                        self.pids.insert(pid, prc.name.clone());
                    }
                    Err(e) => {
                        error!("Can't run container {}: {}", prc.name, e);
                        self.start_queue.push((
                            -((get_time()*1000.) as i64 +
                              prc.restart_timeout),
                            prc.name.clone(),
                            ));
                    }
                }
            }
            Error(_) => {
                self.start_queue.push((
                    -((get_time()*1000.) as i64 +
                        prc.restart_timeout),
                    prc.name.clone(),
                    ));
            }
            _ => {}
        }
        return prepare_result;
    }
    fn _start_processes(&mut self) {
        let time_ms = (get_time() * 1000.) as i64;
        loop {
            let name = match self.start_queue.peek() {
                Some(&(ref ptime, ref name)) if -*ptime < time_ms
                => name.clone(),
                _ => { break; }
            };
            self.start_queue.pop();
            match self._start_process(&name) {
                Run => {}
                Error(e) => {
                    error!("Error preparing container {}: {}", name, e);
                }
                Shutdown => { self.processes.remove(&name); }
            }
        }
    }
    fn _reap_child(&mut self, name: &Rc<String>, pid: pid_t, status: i32)
        -> bool
    {
        let prc = self.processes.get_mut(name).unwrap();
        warn!("[{}] Child {}:{} exited with status {}",
            self.myname, prc.name, pid, status);
        if !prc.executor.finish() {
            return false;
        }
        self.start_queue.push((
            -((prc.start_time.unwrap()*1000.0) as i64 +
                prc.restart_timeout),
            prc.name.clone(),
            ));
        prc.current_pid = None;
        prc.start_time = None;
        return true;
    }
    pub fn run(&mut self) -> MonitorResult {
        debug!("[{}] Starting with {} processes",
            self.myname, self.processes.len());
        // Main loop
        while self.processes.len() > 0 || self.start_queue.len() > 0 {
            let sig = self._wait_signal();
            info!("[{}] Got signal {:?}", self.myname, sig);
            match sig {
                Signal::Timeout => {
                    self._start_processes();
                }
                Signal::Terminate(sig) => {
                    for (_name, prc) in self.processes.iter() {
                        match prc.current_pid {
                            Some(pid) => signal::send_signal(pid, sig),
                            None => {}
                        }
                    }
                    break;
                }
                Signal::Child(pid, status) => {
                    let name = match self.pids.remove(&pid) {
                        Some(name) => name,
                        None => {
                            warn!("[{}] Unknown process {} dead with {}",
                                self.myname, pid, status);
                            continue;
                        },
                    };
                    if !self._reap_child(&name, pid, status) {
                        self.processes.remove(&name);
                    }
                }
                Signal::Reboot => {
                    return MonitorResult::Reboot;
                }
            }
        }
        self.start_queue.clear();
        // Shut down loop
        let mut processes = BTreeMap::new();
        swap(&mut processes, &mut self.processes);
        let mut left: BTreeMap<pid_t, Process> = processes.into_iter()
            .filter(|&(_, ref prc)| prc.current_pid.is_some())
            .map(|(_, prc)| (prc.current_pid.unwrap(), prc))
            .collect();
        info!("[{}] Shutting down, {} processes left",
              self.myname, left.len());
        while left.len() > 0 {
            let sig = self._wait_signal();
            info!("[{}] Got signal {:?}", self.myname, sig);
            match sig {
                Signal::Timeout => { unreachable!(); }
                Signal::Terminate(sig) => {
                    for (_name, prc) in left.iter() {
                        match prc.current_pid {
                            Some(pid) => signal::send_signal(pid, sig),
                            None => {}
                        }
                    }
                }
                Signal::Child(pid, status) => {
                    match left.remove(&pid) {
                        Some(prc) => {
                            info!("[{}] Child {}:{} exited with status {}",
                                self.myname, prc.name, pid, status);
                        }
                        None => {
                            warn!("[{}] Unknown process {} dead with {}",
                                self.myname, pid, status);
                        }
                    }
                }
                Signal::Reboot => {
                    return MonitorResult::Reboot;
                }
            }
        }
        return MonitorResult::Killed;
    }
}
