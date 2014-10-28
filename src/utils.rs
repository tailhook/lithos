use std::io::IoError;
use libc::funcs::posix88::unistd::chdir;
use libc::{c_int, c_char};


extern {
    fn chroot(dir: *const c_char) -> c_int;
}


pub fn temporary_change_root<T>(path: &Path, fun: || -> Result<T, String>)
    -> Result<T, String>
{
    if unsafe { chdir("/".to_c_str().as_ptr()) } != 0 {
        return Err(format!("Error chdir to root: {}",
                           IoError::last_error()));
    }
    if unsafe { chroot(path.to_c_str().as_ptr()) } != 0 {
        return Err(format!("Error chroot to {}: {}",
                           path.display(), IoError::last_error()));
    }
    let res = fun();
    if unsafe { chroot(".".to_c_str().as_ptr()) } != 0 {
        return Err(format!("Error chroot back: {}",
                           IoError::last_error()));
    }
    return res;
}
