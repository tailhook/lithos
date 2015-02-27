use std::os::errno;
use std::cmp::max;
use std::old_io::IoError;
use std::ptr::null;
use std::time::duration::Duration;
use std::default::Default;
use std::old_io::process::Process;
use libc::types::os::common::posix01::timespec;
pub use libc::consts::os::posix88::{SIGTERM, SIGINT, SIGQUIT, EINTR, ECHILD};
pub use libc::consts::os::posix88::{SIGKILL};
use libc::{c_int, pid_t};

use super::utils::{get_time, Time};
use self::Signal::*;


const SIGCHLD: c_int = 17;
const WNOHANG: c_int = 1;

#[derive(Show)]
pub enum Signal {
    Terminate(isize),  // Actual signal for termination: INT, TERM, QUIT...
    Child(pid_t, isize),  //  pid and result code
    Reboot,
    Timeout,  // Not actually a OS signal, but it's a signal for our app
}

#[derive(Default)]
#[repr(C)]
struct CSignalInfo {
    signo: c_int,
    pid: pid_t,
    status: c_int,
}

extern {
    fn block_all_signals();
    fn wait_any_signal(ptr: *mut CSignalInfo, timeout: f64) -> c_int;
    fn waitpid(pid: pid_t, status: *mut c_int, options: c_int) -> pid_t;
}

pub fn block_all() {
    unsafe { block_all_signals() };
}

fn _convert_status(status: i32) -> isize {
    if status & 0xff == 0 {
        return ((status & 0xff00) >> 8) as isize;
    }
    return (128 + (status & 0x7f)) as isize;  // signal
}

pub fn wait_next(reboot_supported: bool, timeout: Option<Time>) -> Signal {
    let mut status: i32 = 0;
    let pid = unsafe { waitpid(-1, &mut status, WNOHANG) };
    if pid > 0 {
        return Child(pid, _convert_status(status));
    }
    loop {
        let mut ptr = Default::default();
        let res = match timeout {
            Some(tm) => {
                let curtime = get_time();
                let dur = if tm > curtime { tm - curtime } else { 0. };
                unsafe { wait_any_signal(&mut ptr, dur) }
            }
            None => {
                unsafe { wait_any_signal(&mut ptr, -1.) }
            }
        };
        if res != 0 {
            //  Any error is ok, because application should be always prepared
            //  for spurious timeouts
            //  only EAGAIN and EINTR expected
            return Timeout;
        }
        match ptr.signo {
            SIGCHLD => {
                loop {
                    status = 0;
                    let rc = unsafe { waitpid(ptr.pid, &mut status, WNOHANG) };
                    if rc < 0 {
                        if errno() == EINTR {
                            continue;
                        }
                        if errno() != ECHILD {
                            panic!("Failure '{}' not expected, on death of {}",
                                IoError::last_error(), ptr.pid);
                        }
                    } else {
                        assert_eq!(rc, ptr.pid);
                        assert_eq!(_convert_status(status), ptr.status as isize);
                    }
                    break;
                }
                return Child(ptr.pid, ptr.status as isize);
            }
            SIGQUIT if reboot_supported => {
                return Reboot;
            }
            sig@SIGTERM | sig@SIGINT | sig@SIGQUIT => {
                return Terminate(sig as isize);
            }
            _ => continue,   // TODO(tailhook) improve logging
        }
    }
}

pub fn send_signal(pid: pid_t, sig: isize) {
    Process::kill(pid, sig).ok();
}

pub fn is_process_alive(pid: pid_t) -> bool {
    return Process::kill(pid, 0).is_ok();
}
