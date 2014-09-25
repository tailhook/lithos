use std::c_str::{CString, ToCStr};
use std::collections::enum_set::{EnumSet,CLike};

enum Namespace {
    NewPid,
}

impl CLike for Namespace {
    fn to_uint(&self) -> uint {
        match *self {
            NewPid => 0,
        }
    }
    fn from_uint(val: uint) -> Namespace {
        match val {
            0 => NewPid,
            _ => unreachable!(),
        }
    }
}

pub struct Command {
    executable: CString,
    arguments: Vec<CString>,
    namespaces: EnumSet<Namespace>,
}

impl Command {
    pub fn new<T:ToCStr>(cmd: T) -> Command {
        return Command {
            executable: cmd.to_c_str(),
            arguments: vec!(cmd.to_c_str()),
            namespaces: EnumSet::empty(),
        };
    }
    pub fn arg<T:ToCStr>(&mut self, arg: T) {
        self.arguments.push(arg.to_c_str());
    }
}
