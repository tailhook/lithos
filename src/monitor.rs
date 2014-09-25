use std::collections::TreeMap;
use libc::pid_t;

use super::container::Command;

pub struct Monitor {
    processes: TreeMap<String, pid_t>,
}

impl Monitor {
    pub fn new() -> Monitor {
        return Monitor {
            processes: TreeMap::new(),
        };
    }
    pub fn add(&mut self, name: String, generator: |&String| -> Command) {
    }
    pub fn wait_all(&mut self) {
    }
}
