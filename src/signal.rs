use std::default::Default;

use libc::consts::os::posix88::SIGTERM;
use libc::{c_int, pid_t};


static SIGCHLD: c_int = 17;

#[deriving(Show)]
pub enum Signal {
    Terminate,
    Child(uint, int),
}

#[deriving(Default)]
struct CSignalInfo {
    signo: c_int,
    pid: pid_t,
    status: c_int,
}

extern {
    fn block_all_signals();
    fn wait_any_signal(ptr: *CSignalInfo);
}

pub fn block_all() {
    unsafe { block_all_signals() };
}

pub fn wait_next() -> Signal {
    loop {
        let ptr = Default::default();
        unsafe { wait_any_signal(&ptr) }
        match ptr.signo {
            SIGTERM => {
                return Terminate;
            }
            SIGCHLD => {
                return Child(ptr.pid as uint, ptr.status as int);
            }
            _ => continue,   // TODO(tailhook) improve logging
        }
    }
}
