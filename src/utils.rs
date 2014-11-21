use std::io::{IoError, PathAlreadyExists};
use std::io::ALL_PERMISSIONS;
use std::io::fs::{mkdir, readdir, rmdir_recursive, rmdir};
use std::io::fs::PathExtensions;
use libc::funcs::posix88::unistd::chdir;
use libc::{c_int, c_char};

use super::tree_config::Range;
use super::container_config::IdMap;


extern {
    fn chroot(dir: *const c_char) -> c_int;
    fn pivot_root(new_root: *const c_char, put_old: *const c_char) -> c_int;
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
    if unsafe { pivot_root(new_root.to_c_str().as_ptr(),
                           put_old.to_c_str().as_ptr()) } != 0 {
        return Err(format!("Error pivot_root to {}: {}", new_root.display(),
                           IoError::last_error()));
    }
    if unsafe { chdir("/".to_c_str().as_ptr()) } != 0 {
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
    }
    match mkdir(dir, ALL_PERMISSIONS) {
        Ok(()) => return Ok(()),
        Err(ref e) if e.kind == PathAlreadyExists => {
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
    // We temporarily change root, so that symlinks inside the dir
    // would do no harm. But note that dir itself can be a symlink
    try!(temporary_change_root(dir, || {
        let dirlist = try!(readdir(&Path::new("/"))
             .map_err(|e| format!("Can't read directory {}: {}",
                                  dir.display(), e)));
        for path in dirlist.into_iter() {
            try!(rmdir_recursive(&path)
                .map_err(|e| format!("Can't remove directory {}{}: {}",
                    dir.display(), path.display(), e)));
        }
        Ok(())
    }));
    if remove_dir_itself {
        try!(rmdir(dir).map_err(|e| format!("Can't remove dir {}: {}",
                                            dir.display(), e)));
    }
    return Ok(());
}
