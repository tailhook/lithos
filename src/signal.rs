use std::ptr::null;
use std::default::Default;
use std::io::process::Process;

pub use libc::consts::os::posix88::{SIGTERM, SIGINT};
use libc::{c_int, pid_t};


static SIGCHLD: c_int = 17;
static WNOHANG: c_int = 1;

#[deriving(Show)]
pub enum Signal {
    Terminate(int),
    Child(pid_t, int),
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
    fn waitpid(pid: pid_t, status: *c_int, options: c_int) -> pid_t;
}

pub fn block_all() {
    unsafe { block_all_signals() };
}

pub fn wait_next() -> Signal {
    let status: i32 = 0;
    let pid = unsafe { waitpid(-1, &status, WNOHANG) };
    if pid > 0 {
        return Child(pid, status as int);
    }
    loop {
        let ptr = Default::default();
        unsafe { wait_any_signal(&ptr) }
        match ptr.signo {
            sig@SIGTERM | sig@SIGINT => {
                return Terminate(sig as int);
            }
            SIGCHLD => {
                unsafe { waitpid(ptr.pid, null(), WNOHANG) };
                return Child(ptr.pid, ptr.status as int);
            }
            _ => continue,   // TODO(tailhook) improve logging
        }
    }
}

pub fn send_signal(pid: pid_t, sig: int) {
    Process::kill(pid, sig).ok();
}
