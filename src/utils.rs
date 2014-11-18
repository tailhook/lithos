use std::io::IoError;
use libc::funcs::posix88::unistd::chdir;
use libc::{c_int, c_char};

use super::tree_config::Range;
use super::container_config::IdMap;


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

pub fn in_range(ranges: &Vec<Range>, value: u32) -> bool {
    if ranges.len() == 0 {  // no limit on the value
        return true;
    }
    for rng in ranges.iter() {
        if rng.start <= value && rng.end >= value {
            return true;
        }
    }
    return false;
}

pub fn check_mapping(ranges: &Vec<Range>, map: &Vec<IdMap>) -> bool {
    // TODO(tailhook) do more comprehensive algo
    'map: for item in map.iter() {
        for rng in ranges.iter() {
            if rng.start <= item.outside &&
                rng.end >= item.outside + item.count - 1
            {
                continue 'map;
            }
        }
        return false;
    }
    return true;
}
