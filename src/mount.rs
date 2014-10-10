#![allow(dead_code)]
use std::io::{IoError, EndOfFile};
use std::io::BufferedReader;
use std::ptr::null;
use std::io::fs::File;
use libc::{c_ulong, c_int};

// sys/mount.h
static MS_RDONLY: c_ulong = 1;                /* Mount read-only.  */
static MS_NOSUID: c_ulong = 2;                /* Ignore suid and sgid bits.  */
static MS_NODEV: c_ulong = 4;                 /* Disallow access to device special files.  */
static MS_NOEXEC: c_ulong = 8;                /* Disallow program execution.  */
static MS_SYNCHRONOUS: c_ulong = 16;          /* Writes are synced at once.  */
static MS_REMOUNT: c_ulong = 32;              /* Alter flags of a mounted FS.  */
static MS_MANDLOCK: c_ulong = 64;             /* Allow mandatory locks on an FS.  */
static MS_DIRSYNC: c_ulong = 128;             /* Directory modifications are synchronous.  */
static MS_NOATIME: c_ulong = 1024;            /* Do not update access times.  */
static MS_NODIRATIME: c_ulong = 2048;         /* Do not update directory access times.  */
static MS_BIND: c_ulong = 4096;               /* Bind directory at different place.  */
static MS_MOVE: c_ulong = 8192;
static MS_REC: c_ulong = 16384;
static MS_SILENT: c_ulong = 32768;
static MS_POSIXACL: c_ulong = 1 << 16;        /* VFS does not apply the umask.  */
static MS_UNBINDABLE: c_ulong = 1 << 17;      /* Change to unbindable.  */
static MS_PRIVATE: c_ulong = 1 << 18;         /* Change to private.  */
static MS_SLAVE: c_ulong = 1 << 19;           /* Change to slave.  */
static MS_SHARED: c_ulong = 1 << 20;          /* Change to shared.  */
static MS_RELATIME: c_ulong = 1 << 21;        /* Update atime relative to mtime/ctime.  */
static MS_KERNMOUNT: c_ulong = 1 << 22;       /* This is a kern_mount call.  */
static MS_I_VERSION: c_ulong =  1 << 23;      /* Update inode I_version field.  */
static MS_STRICTATIME: c_ulong = 1 << 24;     /* Always perform atime updates.  */
static MS_ACTIVE: c_ulong = 1 << 30;
static MS_NOUSER: c_ulong = 1 << 31;


extern {
    fn mount(source: *u8, target: *u8,
        filesystemtype: *u8, flags: c_ulong,
        data: *u8) -> c_int;
    fn umount(target: *u8) -> c_int;
}

pub fn mount_ro_recursive(target: &Path) -> Result<(), String> {
    let none = "none".to_c_str();
    //  Must recursively remount readonly
    //  TODO(tailhook) fix double and overlapping bind mounts
    let file = try_str!(File::open(&Path::new("/proc/mounts")));
    let mut buf = BufferedReader::new(file);
    loop {
        let line = match buf.read_line() {
            Ok(line) => line,
            Err(ref e) if e.kind == EndOfFile => break,
            Err(e) => {
                return Err(format!("Error reading /proc/mounts: {}", e));
            }
        };
        let mut iter = line.as_slice().splitn(' ', 2);
        let cur_source = match iter.next() {
            Some(src) => src,
            None => {
                warn!("Wrong line in /proc/mounts: {}", line);
                continue;
            }
        };
        let cur_target = match iter.next() {
            Some(target) => Path::new(target),
            None => {
                warn!("Wrong line in /proc/mounts: {}", line);
                continue;
            }
        };
        if cur_source.as_slice() == "cgroup" {
            // Can't remount readonly cgroup filesystem
            continue;
        }
        if target.is_ancestor_of(&cur_target) {
            let c_target = cur_target.to_c_str();
            debug!("Remount readonly {} ({})",
                cur_target.display(), cur_source);
            let rc = unsafe { mount(
               none.as_bytes().as_ptr(),
               c_target.as_bytes().as_ptr(),
               null(), MS_BIND|MS_REMOUNT|MS_RDONLY, null()) };
            if rc != 0 {
                let err = IoError::last_error();
                return Err(format!("Remount readonly {}: {}",
                    cur_target.display(), err));
            }
            try_str!(mount_private(&cur_target));
        }
    }
    return Ok(());
}

pub fn mount_private(target: &Path) -> Result<(), String> {
    let none = "none".to_c_str();
    let c_target = target.to_c_str();
    debug!("Making private {}", target.display());
    let rc = unsafe { mount(
        none.as_bytes().as_ptr(),
        c_target.as_bytes().as_ptr(),
        null(), MS_PRIVATE, null()) };
    if rc == 0 {
        return Ok(());
    } else {
        let err = IoError::last_error();
        return Err(format!("Can't make {} a slave: {}",
            target.display(), err));
    }
}

pub fn bind_mount(source: &Path, target: &Path) -> Result<(), String> {
    let c_source = source.to_c_str();
    let c_target = target.to_c_str();
    debug!("Bind mount {} -> {}", source.display(), target.display());
    let rc = unsafe {
        mount(c_source.as_bytes().as_ptr(), c_target.as_bytes().as_ptr(),
        null(), MS_BIND|MS_REC, null()) };
    if rc == 0 {
        return Ok(());
    } else {
        let err = IoError::last_error();
        return Err(format!("Can't mount bind {} to {}: {}",
            source.display(), target.display(), err));
    }
}

pub fn mount_pseudo(target: &Path, name: &str, options: &str, readonly: bool)
    -> Result<(), String>
{
    let c_name = name.to_c_str();
    let c_target = target.to_c_str();
    let c_opts = options.to_c_str();
    let mut flags = MS_NOSUID | MS_NOEXEC | MS_NODEV | MS_NOATIME;
    if readonly {
        flags |= MS_RDONLY;
    }
    debug!("Pseusofs mount {} {} {}", target.display(), name, options);
    let rc = unsafe { mount(
        c_name.as_bytes().as_ptr(),
        c_target.as_bytes().as_ptr(),
        c_name.as_bytes().as_ptr(),
        flags,
        c_opts.as_bytes().as_ptr()) };
    if rc == 0 {
        return Ok(());
    } else {
        let err = IoError::last_error();
        return Err(format!("Can't mount pseudofs {} ({}, options: {}): {}",
            target.display(), options, name, err));
    }
}

pub fn mount_tmpfs(target: &Path, options: &str) -> Result<(), String> {
    let c_tmpfs = "tmpfs".to_c_str();
    let c_target = target.to_c_str();
    let c_opts = options.to_c_str();
    debug!("Tmpfs mount {} {}", target.display(), options);
    let rc = unsafe { mount(
        c_tmpfs.as_bytes().as_ptr(),
        c_target.as_bytes().as_ptr(),
        c_tmpfs.as_bytes().as_ptr(),
        MS_NOSUID | MS_NODEV | MS_NOATIME,
        c_opts.as_bytes().as_ptr()) };
    if rc == 0 {
        return Ok(());
    } else {
        let err = IoError::last_error();
        return Err(format!("Can't mount tmpfs {} (options: {}): {}",
            target.display(), options, err));
    }
}

pub fn unmount(target: &Path) -> Result<(), String> {
    let c_target = target.to_c_str();
    let rc = unsafe { umount(c_target.as_bytes().as_ptr()) };
    if rc == 0 {
        return Ok(());
    } else {
        let err = IoError::last_error();
        return Err(format!("Can't unmount {} : {}", target.display(), err));
    }
}
