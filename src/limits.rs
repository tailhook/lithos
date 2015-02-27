use std::old_io::IoError;
use libc::c_int;

static RLIMIT_NOFILE: c_int = 7;

#[repr(C)]
struct rlimit {
    rlim_cur: u64,
    rlim_max: u64,
}

extern "C" {
    fn setrlimit(resource: c_int, rlimit: *const rlimit) -> c_int;
}

pub fn set_fileno_limit(limit: u64) -> Result<(), IoError> {
    let res = unsafe { setrlimit(RLIMIT_NOFILE, &rlimit {
        rlim_cur: limit,
        rlim_max: limit,
    }) };
    if res != 0 {
        return Err(IoError::last_error());
    }
    return Ok(());
}
