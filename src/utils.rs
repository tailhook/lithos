use std::ptr;
use std::fs::create_dir;
use std::path::{Path, PathBuf};
use std::io::Error as IoError;
use std::io::ErrorKind::AlreadyExists;
use std::ffi::CString;
use std::env::current_dir;
use libc::{c_int, c_char, timeval, c_void, mode_t};
use libc::{chmod, chdir};

use super::tree_config::Range;
use super::container_config::IdMap;

pub type Time = f64;


extern {
    fn chroot(dir: *const c_char) -> c_int;
    fn pivot_root(new_root: *const c_char, put_old: *const c_char) -> c_int;
    fn gettimeofday(tp: *mut timeval, tzp: *mut c_void) -> c_int;
}


pub fn temporary_change_root<T, F>(path: &Path, fun: F)
    -> Result<T, String>
    where F: Fn() -> Result<T, String>
{
    let cwd = current_dir().unwrap();
    if unsafe { chdir(CString::from_slice("/".as_bytes()).as_ptr()) } != 0 {
        return Err(format!("Error chdir to root: {}",
                           IoError::last_error()));
    }
    if unsafe { chroot(CString::from_slice(path.container_as_bytes()).as_ptr()) } != 0 {
        return Err(format!("Error chroot to {}: {}",
                           path.display(), IoError::last_error()));
    }
    let res = fun();
    if unsafe { chroot(CString::from_slice(".".as_bytes()).as_ptr()) } != 0 {
        return Err(format!("Error chroot back: {}",
                           IoError::last_error()));
    }
    if unsafe { chdir(CString::from_slice(cwd.container_as_bytes()).as_ptr()) } != 0 {
        return Err(format!("Error chdir to workdir back: {}",
                           IoError::last_error()));
    }
    return res;
}

pub fn in_range(ranges: &Vec<Range>, value: u32) -> bool {
    for rng in ranges.iter() {
        if rng.start <= value && rng.end >= value {
            return true;
        }
    }
    return false;
}

pub fn in_mapping(mapping: &Vec<IdMap>, value: u32) -> bool {
    for mp in mapping.iter() {
        if value >= mp.inside && value < mp.inside + mp.count {
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

pub fn change_root(new_root: &Path, put_old: &Path) -> Result<(), String>
{
    if unsafe { pivot_root(
            CString::from_slice(new_root.container_as_bytes()).as_ptr(),
            CString::from_slice(put_old.container_as_bytes()).as_ptr()) } != 0
    {
        return Err(format!("Error pivot_root to {}: {}", new_root.display(),
                           IoError::last_error()));
    }
    if unsafe { chdir(
            CString::from_slice("/".as_bytes()).as_ptr()) } != 0
    {
        return Err(format!("Error chdir to root: {}",
                           IoError::last_error()));
    }
    return Ok(());
}

pub fn ensure_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        if !dir.is_dir() {
            return Err(format!(concat!("Can't create dir {}, ",
                "path already exists but not a directory"),
                dir.display()));
        }
        return Ok(());
    }

    match create_dir(dir) {
        Ok(()) => return Ok(()),
        Err(ref e) if e.kind == AlreadyExists => {
            if dir.is_dir() {
                return Ok(());
            } else {
                return Err(format!(concat!("Can't create dir {}, ",
                    "path already exists but not a directory"),
                    dir.display()));
            }
        }
        Err(ref e) => {
            return Err(format!(concat!("Can't create dir {}: {} ",
                "path already exists but not a directory"),
                dir.display(), e));
        }
    }
}

pub fn clean_dir(dir: &Path, remove_dir_itself: bool) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    // We temporarily change root, so that symlinks inside the dir
    // would do no harm. But note that dir itself can be a symlink
    try!(temporary_change_root(dir, || {
        let dirlist = try!(read_dir(&PathBuf::new("/"))
             .map_err(|e| format!("Can't read directory {}: {}",
                                  dir.display(), e)))
             .collect();
        for path in dirlist.into_iter() {
            if path.is_dir() {
                try!(rmdir_recursive(&path)
                    .map_err(|e| format!("Can't remove directory {}{}: {}",
                        dir.display(), path.display(), e)));
            } else {
                try!(unlink(&path)
                    .map_err(|e| format!("Can't remove file {}{}: {}",
                        dir.display(), path.display(), e)));
            }
        }
        Ok(())
    }));
    if remove_dir_itself {
        try!(rmdir(dir).map_err(|e| format!("Can't remove dir {}: {}",
                                            dir.display(), e)));
    }
    return Ok(());
}

pub fn join<T: Str, I: Iterator<Item=T>>(array: I, delimiter: &str) -> String {
    let mut array = array;
    let mut res = "".to_string();
    match array.next() {
        Some(x) => {
            res.push_str(x.as_slice());
            for name in array {
                res.push_str(delimiter);
                res.push_str(name.as_slice());
            }
        }
        None => {}
    }
    return res;
}

pub fn get_time() -> Time {
    let mut tv = timeval { tv_sec: 0, tv_usec: 0 };
    unsafe { gettimeofday(&mut tv, ptr::null_mut()) };
    return (tv.tv_sec as f64 +  tv.tv_usec as f64 * 0.000001)
}

pub fn set_file_mode(path: &Path, mode: mode_t) -> Result<(), IoError> {
    let cpath = CString::from_slice(path.container_as_bytes());
    let rc = unsafe { chmod(cpath.as_ptr(), mode) };
    if rc < 0 {
        return Err(IoError::last_error());
    }
    return Ok(());
}
