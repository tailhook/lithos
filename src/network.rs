use std::str::from_utf8;
use std::ptr::null;
use std::ffi::CString;
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::io::ErrorKind::InvalidInput;
use std::net::IpAddr;
use libc::{c_int, size_t, c_char};

#[repr(C)]
struct hostent {
    h_name: *const c_char,              /* official name of host */
    h_aliases: *const *const c_char,    /* alias list */
    h_addrtype: c_int,                  /* host address type */
    h_length: c_int,                    /* length of address */
    h_addr_list: *const *const c_char,  /* list of addresses */
}

extern {
    pub fn gethostname(name: *mut c_char, size: size_t) -> c_int;
    pub fn gethostbyname(name: *const c_char) -> *const hostent;
}

pub fn get_host_ip() -> IoResult<IpAddr> {
    let host = try!(get_host_name());
    let addr = try!(get_host_address(host.as_slice()));
    return Ok(addr[0]);
}

pub fn get_host_name() -> IoResult<String> {
    let mut buf: Vec<u8> = Vec::with_capacity(256);
    let nbytes = unsafe {
        buf.set_len(256);
        gethostname(
            buf.as_mut_slice().as_mut_ptr() as *mut i8,
            256)
    };
    if nbytes != 0 {
        return Err(IoError::last_error());
    }
    return buf.as_slice().splitn(1, |x| *x == 0u8)
           .next()
           .and_then(|x| String::from_utf8(x.to_vec()).ok())
           .ok_or(IoError {
                kind: InvalidInput,
                desc: "Got invalid hostname from OS",
                detail: None,
            });
}

pub fn get_host_address(val: &str) -> IoResult<String> {
    let cval = CString::new(val);
    unsafe {
        let hostent = gethostbyname(cval.as_ptr());
        if hostent == null() {
            return Err(IoError::last_error());
        }
        if hostent.h_length == 0 {
            return Err(IoError::from_raw_os_error(InvalidInput));
        }
        return Ok(CString::from_ptr(hostent.h_addr_list[0]));
    }
}
