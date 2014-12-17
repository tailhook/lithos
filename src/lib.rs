#![crate_name="lithos"]
#![crate_type="lib"]

#![feature(macro_rules,phase,if_let,slicing_syntax)]

#[phase(plugin, link)] extern crate log;
extern crate debug;
extern crate libc;
extern crate quire;
extern crate serialize;
extern crate time;
extern crate regex;
#[phase(plugin)] extern crate regex_macros;

pub mod master_config;
pub mod tree_config;
pub mod container_config;
pub mod child_config;
pub mod monitor;
pub mod container;
pub mod macros;
pub mod signal;
pub mod mount;
pub mod utils;
pub mod network;
pub mod setup;
pub mod pipe;
pub mod limits;
pub mod sha256;
pub mod cgroup;
#[cfg(test)] pub mod ascii;  // actually a lithos_ps module
