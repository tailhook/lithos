use std::io::Error as IoError;
use std::io::ErrorKind::BrokenPipe;
use std::os::unix::io::RawFd;
use nix::unistd::{pipe};

use libc::{c_int, c_void};
use libc::funcs::posix88::unistd::{close, write};
use libc::consts::os::posix88::{EINTR, EAGAIN};


pub struct CPipe {
    reader: RawFd,
    writer: RawFd,
}

impl CPipe {
    pub fn new() -> Result<CPipe, IoError> {
        match unsafe { pipe() } {
            Ok((reader, writer)) => Ok(CPipe {
                reader: reader, writer: writer
            }),
            Err(e) => Err(e),
        }
    }
    pub fn reader_fd(&self) -> c_int {
        return self.reader;
    }
    pub fn wakeup(&self) -> Result<(), IoError> {
        let mut rc;
        loop {
            unsafe {
                rc = write(self.writer,
                    ['x' as u8].as_ptr() as *const c_void, 1);
            }
            let err = IoError::last_os_error().raw_os_error();
            if rc < 0 && (err == Some(EINTR) || err == Some(EAGAIN)) {
                continue
            }
            break;
        }
        if rc == 0 {
            return Err(IoError { kind: BrokenPipe, detail: None,
                desc: "Pipe was closed. Probably process is dead"});
        } else if rc == -1 {
            return Err(IoError::last_error());
        }
        return Ok(());
    }
}

impl Drop for CPipe {
    fn drop(&mut self) {
        unsafe {
            close(self.reader);
            close(self.writer);
        }
    }
}
