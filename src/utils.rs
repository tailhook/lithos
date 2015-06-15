use std::ptr;
use std::fs::{create_dir, remove_dir_all, read_dir, remove_file, remove_dir};
use std::fs::PathExt;
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
    if unsafe { chdir(CString::new("/").unwrap().as_ptr()) } != 0 {
        return Err(format!("Error chdir to root: {}",
                           IoError::last_os_error()));
    }
    if unsafe { chroot(cpath(&path).as_ptr()) } != 0 {
        return Err(format!("Error chroot to {:?}: {}",
                           path, IoError::last_os_error()));
    }
    let res = fun();
    if unsafe { chroot(CString::new(".").unwrap().as_ptr()) } != 0 {
        return Err(format!("Error chroot back: {}",
                           IoError::last_os_error()));
    }
    if unsafe { chdir(cpath(&cwd).as_ptr()) } != 0 {
        return Err(format!("Error chdir to workdir back: {}",
                           IoError::last_os_error()));
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
            cpath(new_root).as_ptr(),
            cpath(put_old).as_ptr()) } != 0
    {
        return Err(format!("Error pivot_root to {}: {}", new_root.display(),
                           IoError::last_os_error()));
    }
    if unsafe { chdir(CString::new("/").unwrap().as_ptr()) } != 0
    {
        return Err(format!("Error chdir to root: {}",
                           IoError::last_os_error()));
    }
    return Ok(());
}

pub fn ensure_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        if !dir.is_dir() {
            return Err(format!(concat!("Can't create dir {:?}, ",
                "path already exists but not a directory"), dir));
        }
        return Ok(());
    }

    match create_dir(dir) {
        Ok(()) => return Ok(()),
        Err(ref e) if e.kind() == AlreadyExists => {
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
        let dirlist = try!(read_dir("/")
             .map_err(|e| format!("Can't read directory {:?}: {}", dir, e)))
             .filter_map(|x| x.ok())
             .collect();
        for path in dirlist.into_iter() {
            if path.is_dir() {
                try!(remove_dir_all(&path)
                    .map_err(|e| format!("Can't remove directory {:?}{:?}: {}",
                        dir, path, e)));
            } else {
                try!(remove_file(&path)
                    .map_err(|e| format!("Can't remove file {:?}{:?}: {}",
                        dir, path, e)));
            }
        }
        Ok(())
    }));
    if remove_dir_itself {
        try!(remove_dir(dir)
            .map_err(|e| format!("Can't remove dir {:?}: {}", dir, e)));
    }
    return Ok(());
}

pub fn join<S1, S2, I>(mut iter: I, sep: S2) -> String
    where S1:AsRef<str>, S2:AsRef<str>, I:Iterator<Item=S1>
{
    let mut buf = String::new();
    match iter.next() {
        Some(x) => buf.push_str(x.as_ref()),
        None => {}
    }
    for i in iter {
        buf.push_str(sep.as_ref());
        buf.push_str(i.as_ref());
    }
    return buf;
}

pub fn get_time() -> Time {
    let mut tv = timeval { tv_sec: 0, tv_usec: 0 };
    unsafe { gettimeofday(&mut tv, ptr::null_mut()) };
    return (tv.tv_sec as f64 +  tv.tv_usec as f64 * 0.000001)
}

pub fn set_file_mode(path: &Path, mode: mode_t) -> Result<(), IoError> {
    let cpath = CString::new(path).unwrap();
    let rc = unsafe { chmod(cpath.as_ptr(), mode) };
    if rc < 0 {
        return Err(IoError::last_error());
    }
    return Ok(());
}

pub fn cpath(path: &Path) -> CString {
    CString::new(path.to_str().unwrap()).unwrap()
}

