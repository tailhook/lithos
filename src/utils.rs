use std::ptr;
use std::io;
use std::fs::{create_dir, remove_dir_all, read_dir, remove_file, remove_dir};
use std::fs::{metadata};
use std::path::{Path, PathBuf};
use std::path::Component::Normal;
use std::io::Error as IoError;
use std::io::ErrorKind::{AlreadyExists, NotFound};
use std::ffi::CString;
use std::env::current_dir;

use nix::sys::signal::Signal;
use nix::sys::signal::{SIGQUIT, SIGSEGV, SIGBUS, SIGHUP, SIGILL, SIGABRT};
use nix::sys::signal::{SIGFPE, SIGUSR1, SIGUSR2};
use libc::{c_int, c_char, timeval, c_void, mode_t, uid_t, gid_t};
use libc::{chmod, chdir, chown};
use signal::trap::Trap;
use range::Range;


use super::id_map::IdMap;

pub type Time = f64;
pub type SigNum = i32;
// TODO(tailhook) signal::Trap might use nix signals instead of i32
pub const ABNORMAL_TERM_SIGNALS: &'static [Signal] = &[
    SIGQUIT, SIGSEGV, SIGBUS, SIGHUP,
    SIGILL, SIGABRT, SIGFPE, SIGUSR1,
    SIGUSR2,
];

pub struct FsUidGuard(bool);

extern {
    fn chroot(dir: *const c_char) -> c_int;
    fn pivot_root(new_root: *const c_char, put_old: *const c_char) -> c_int;
    fn gettimeofday(tp: *mut timeval, tzp: *mut c_void) -> c_int;

    // TODO(tailhook) move to libc and nix
    fn setfsuid(uid: uid_t) -> c_int;
    fn setfsgid(gid: gid_t) -> c_int;
}


pub fn temporary_change_root<T, F>(path: &Path, mut fun: F)
    -> Result<T, String>
    where F: FnMut() -> Result<T, String>
{
    // The point is: if we gat fatal signal in the chroot, we have 2 issues:
    //
    // 1. Process can't actually restart (the binary path is wrong)
    // 2. Even if it finds the binary, it will be angry restarting in chroot
    //
    let _trap = Trap::trap(ABNORMAL_TERM_SIGNALS);

    let cwd = current_dir().map_err(|e| {
        format!("Can't determine current dir: {}. \
            This usually happens if the directory \
            your're in is already deleted", e)
    })?;
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
    if let Ok(dmeta) = metadata(dir) {
        if !dmeta.is_dir() {
            return Err(format!(concat!("Can't create dir {:?}, ",
                "path already exists but not a directory"), dir));
        }
        return Ok(());
    }

    match create_dir(dir) {
        Ok(()) => return Ok(()),
        Err(ref e) if e.kind() == AlreadyExists => {
            let dmeta = metadata(dir);
            if dmeta.is_ok() && dmeta.unwrap().is_dir() {
                return Ok(());
            } else {
                return Err(format!(concat!("Can't create dir {:?}, ",
                    "path already exists but not a directory"),
                    dir));
            }
        }
        Err(ref e) => {
            return Err(format!(concat!("Can't create dir {:?}: {} ",
                "path already exists but not a directory"), dir, e));
        }
    }
}

pub fn clean_dir(dir: &Path, remove_dir_itself: bool) -> Result<(), String> {
    if let Err(e) = metadata(dir) {
        if e.kind() == NotFound {
            return Ok(());
        } else {
            return Err(format!("Can't stat dir {:?}: {}", dir, e));
        }
    }
    // We temporarily change root, so that symlinks inside the dir
    // would do no harm. But note that dir itself can be a symlink
    try!(temporary_change_root(dir, || {
        let dirlist = try!(read_dir("/")
             .map_err(|e| format!("Can't read directory {:?}: {}", dir, e)))
             .filter_map(|x| x.ok())
             .collect::<Vec<_>>();
        for entry in dirlist.into_iter() {
            match metadata(entry.path()) {
                Ok(ref meta) if meta.is_dir() => {
                    try!(remove_dir_all(entry.path())
                        .map_err(|e| format!("Can't remove directory {:?}{:?}: {}",
                            dir, entry.path(), e)));
                }
                Ok(_) => {
                    try!(remove_file(entry.path())
                        .map_err(|e| format!("Can't remove file {:?}{:?}: {}",
                            dir, entry.path(), e)));
                }
                Err(_) => {
                    return Err(format!("Can't stat file {:?}", entry.path()));
                }
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
    return tv.tv_sec as f64 +  tv.tv_usec as f64 * 0.000001;
}

pub fn set_file_owner(path: &Path, owner: uid_t, group: gid_t)
    -> Result<(), IoError>
{
    let cpath = cpath(path);
    let rc = unsafe { chown(cpath.as_ptr(), owner, group) };
    if rc < 0 {
        return Err(IoError::last_os_error());
    }
    return Ok(());
}

pub fn set_file_mode(path: &Path, mode: mode_t) -> Result<(), IoError> {
    let cpath = cpath(path);
    let rc = unsafe { chmod(cpath.as_ptr(), mode) };
    if rc < 0 {
        return Err(IoError::last_os_error());
    }
    return Ok(());
}

pub fn cpath<P:AsRef<Path>>(path: P) -> CString {
    CString::new(path.as_ref().to_str().unwrap()).unwrap()
}

pub fn relative(child: &Path, base: &Path) -> PathBuf {
    assert!(child.starts_with(base));
    let mut res = PathBuf::new();
    for cmp in child.components().skip(base.components().count()) {
        if let Normal(ref chunk) = cmp {
            res.push(chunk);
        } else {
            panic!("Bad path for relative ({:?} from {:?} against {:?})",
                cmp, child, base);
        }
    }
    return res
}

impl FsUidGuard {
    pub fn set(uid: u32, gid: u32) -> FsUidGuard {
        if uid != 0 || gid != 0 {
            unsafe { setfsuid(uid) };
            if unsafe { setfsuid(uid) } != uid as i32 {
                error!("Can't set fs gid to open socket: {}. Ignoring.",
                    io::Error::last_os_error());
            }
            unsafe { setfsgid(gid) };
            if unsafe { setfsgid(gid) } != gid as i32 {
                error!("Can't set fs uid to open socket: {}. Ignoring.",
                    io::Error::last_os_error());
            }
            FsUidGuard(true)
        } else {
            FsUidGuard(false)
        }
    }
}

impl Drop for FsUidGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { setfsuid(0) };
            if unsafe { setfsuid(0) } != 0 {
                let err = io::Error::last_os_error();
                error!("Can't return fs uid back to zero: {}. Aborting.", err);
                panic!("Can't return fs uid back to zero: {}. Aborting.", err);
            }
            unsafe { setfsgid(0) };
            if unsafe { setfsgid(0) } != 0 {
                let err = io::Error::last_os_error();
                error!("Can't return fs gid back to zero: {}. Aborting.", err);
                panic!("Can't return fs gid back to zero: {}. Aborting.", err);
            }
        }
    }
}
