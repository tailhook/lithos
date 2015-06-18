use std::io::Error as IoError;
use std::default::Default;
pub use libc::consts::os::posix88::{SIGTERM, SIGINT, SIGQUIT, EINTR, ECHILD};
pub use libc::consts::os::posix88::{SIGKILL};
use libc::{c_int, pid_t};
use nix::sys::signal::kill;

use super::utils::{get_time, Time};
use self::Signal::*;


const SIGCHLD: c_int = 17;
const WNOHANG: c_int = 1;

#[derive(Debug)]
pub enum Signal {
    Terminate(i32),  // Actual signal for termination: INT, TERM, QUIT...
    Child(pid_t, i32),  //  pid and result code
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

fn _convert_status(status: i32) -> i32 {
    if status & 0xff == 0 {
        return (status & 0xff00) >> 8;
    }
    return 128 + (status & 0x7f);  // signal
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
                        let err = IoError::last_os_error().raw_os_error();
                        if err == Some(EINTR) {
                            continue;
                        }
                        if err != Some(ECHILD) {
                            panic!("Failure '{}' not expected, on death of {}",
                                IoError::last_os_error(), ptr.pid);
                        }
                    } else {
                        assert_eq!(rc, ptr.pid);
                        assert_eq!(_convert_status(status), ptr.status);
                    }
                    break;
                }
                return Child(ptr.pid, ptr.status);
            }
            SIGQUIT if reboot_supported => {
                return Reboot;
            }
            sig@SIGTERM | sig@SIGINT | sig@SIGQUIT => {
                return Terminate(sig as i32);
            }
            _ => continue,   // TODO(tailhook) improve logging
        }
    }
}

pub fn send_signal(pid: pid_t, sig: i32) {
    kill(pid, sig).ok();
}

pub fn is_process_alive(pid: pid_t) -> bool {
    return kill(pid, 0).is_ok();
}
