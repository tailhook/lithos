use std::collections::HashMap;

use libcantal::{Counter, Integer, Collection, Visitor, Name, NameVisitor};


pub struct Process {
    pub started: Counter,
    pub failures: Counter,
    pub deaths: Counter,
    pub running: Integer,
}

pub struct Metrics {
    pub restarts: Counter,
    pub sandboxes: Integer,
    pub containers: Integer,
    pub queue: Integer,

    pub started: Counter,
    pub failures: Counter,
    pub deaths: Counter,
    pub running: Integer,
    pub unknown: Integer,

    pub processes: HashMap<(String, String), Process>,
}

pub struct MasterName(&'static str);
pub struct GlobalName(&'static str);
pub struct ProcessName<'a>(&'a str, &'a str, &'static str);

impl Metrics {
    pub fn new() -> Metrics {
        Metrics {
            restarts: Counter::new(),
            sandboxes: Integer::new(),
            containers: Integer::new(),

            started: Counter::new(),
            failures: Counter::new(),
            deaths: Counter::new(),
            running: Integer::new(),
            unknown: Integer::new(),
            queue: Integer::new(),

            processes: HashMap::new(),
        }
    }
}

impl Process {
    pub fn new() -> Process {
        Process {
            started: Counter::new(),
            failures: Counter::new(),
            deaths: Counter::new(),
            running: Integer::new(),
        }
    }
}


impl Collection for Metrics {
    fn visit<'x>(&'x self, visitor: &mut Visitor<'x>) {
        visitor.metric(&MasterName("restarts"), &self.restarts);
        visitor.metric(&MasterName("sandboxes"), &self.sandboxes);
        visitor.metric(&MasterName("containers"), &self.containers);
        visitor.metric(&MasterName("queue"), &self.queue);

        visitor.metric(&GlobalName("started"), &self.started);
        visitor.metric(&GlobalName("failures"), &self.failures);
        visitor.metric(&GlobalName("deaths"), &self.deaths);
        visitor.metric(&GlobalName("running"), &self.running);
        for (&(ref g, ref n), ref p) in &self.processes {
            visitor.metric(&ProcessName(g, n, "started"), &p.started);
            visitor.metric(&ProcessName(g, n, "failures"), &p.failures);
            visitor.metric(&ProcessName(g, n, "deaths"), &p.deaths);
            visitor.metric(&ProcessName(g, n, "running"), &p.running);
        }
    }
}

impl Name for MasterName {
    fn get(&self, key: &str) -> Option<&str> {
        match key {
            "group" => Some("master"),
            "metric" => Some(self.0),
            _ => None,
        }
    }
    fn visit(&self, s: &mut NameVisitor) {
        s.visit_pair("group", "master");
        s.visit_pair("metric", self.0);
    }
}

impl Name for GlobalName {
    fn get(&self, key: &str) -> Option<&str> {
        match key {
            "group" => Some("containers"),
            "metric" => Some(self.0),
            _ => None,
        }
    }
    fn visit(&self, s: &mut NameVisitor) {
        s.visit_pair("group", "containers");
        s.visit_pair("metric", self.0);
    }
}

impl<'a> Name for ProcessName<'a> {
    fn get(&self, _key: &str) -> Option<&str> {
        unimplemented!();
    }
    fn visit(&self, s: &mut NameVisitor) {
        s.visit_pair("group", &format!("processes.{}.{}", self.0, self.1));
        s.visit_pair("metric", self.2);
    }
}
