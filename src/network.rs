use std::ptr::{null, copy};
use std::ffi::{CString};
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::net::Ipv4Addr;
use libc::{c_int, size_t, c_char, EINVAL};

#[repr(C)]
struct hostent {
    h_name: *const c_char,              /* official name of host */
    h_aliases: *const *const c_char,    /* alias list */
    h_addrtype: c_int,                  /* host address type */
    h_length: c_int,                    /* length of address */
    h_addr_list: *const *const c_char,  /* list of addresses */
}

extern {
    fn gethostname(name: *mut c_char, size: size_t) -> c_int;
    fn gethostbyname(name: *const c_char) -> *const hostent;
}

pub fn get_host_ip() -> IoResult<String> {
    let host = try!(get_host_name());
    let addr = try!(get_host_address(&host[..]));
    return Ok(addr);
}

pub fn get_host_name() -> IoResult<String> {
    let mut buf: Vec<u8> = Vec::with_capacity(256);
    let nbytes = unsafe {
        buf.set_len(256);
        gethostname(
            (&mut buf[..]).as_ptr() as *mut i8,
            256)
    };
    if nbytes != 0 {
        return Err(IoError::last_os_error());
    }
    return buf[..].splitn(2, |x| *x == 0u8)
           .next()
           .and_then(|x| String::from_utf8(x.to_vec()).ok())
           .ok_or(IoError::from_raw_os_error(EINVAL));
}

pub fn get_host_address(val: &str) -> IoResult<String> {
    let cval = CString::new(val).unwrap();
    unsafe {
        let hostent = gethostbyname(cval.as_ptr());
        if hostent == null() {
            return Err(IoError::last_os_error());
        }
        if (*hostent).h_length == 0 {
            return Err(IoError::from_raw_os_error(EINVAL));
        }
        let mut addr = [0u8; 4];
        copy(*(*hostent).h_addr_list, addr.as_mut_ptr() as *mut i8, 4);
        return Ok(format!("{}",
                  Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3])));
    }
}
