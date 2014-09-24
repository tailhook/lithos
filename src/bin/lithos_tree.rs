#![feature(phase, macro_rules)]

extern crate serialize;
#[phase(plugin, link)] extern crate log;

extern crate argparse;
extern crate quire;


use std::rc::Rc;
use std::io::stderr;
use std::io::fs::File;
use std::os::set_exit_status;
use std::default::Default;
use std::collections::TreeMap;
use serialize::Decodable;

use argparse::{ArgumentParser, Store};
use quire::parser::parse;
use quire::ast::process;
use quire::decode::YamlDecoder;

use lithos::tree_config::TreeConfig;

#[path="../mod.rs"]
mod lithos;

macro_rules! try_str {
    ($expr:expr) => {
        try!(($expr).map_err(|e| format!("{}: {}", stringify!($expr), e)))
    }
}


fn run(config_file: Path) -> Result<(), String> {
    let mut file = try_str!(File::open(&config_file));
    let body = try_str!(file.read_to_str());
    let (ast, warnings) = try_str!(parse(
        Rc::new(config_file.display().as_maybe_owned().into_string()),
        body.as_slice(),
        |doc| { process(Default::default(), doc) }));
    if warnings.len() > 0 {
        (write!(stderr(), "Warnings: {}", warnings.len())).ok();
        return Err("Error parsing configuration file".to_string());
    }

    let mut dec = YamlDecoder::new(ast);
    let cfg: TreeConfig = try_str!(Decodable::decode(&mut dec));

    return Ok(());
}


fn main() {
    let mut config_file = Path::new("/etc/lithos.yaml");
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Runs tree of processes");
        ap.refer(&mut config_file)
          .add_option(["-C", "--config"], box Store::<Path>,
            "Name of the global configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => {
                set_exit_status(x);
                return;
            }
        }
    }
    match run(config_file) {
        Ok(()) => {
            set_exit_status(0);
        }
        Err(e) => {
            (write!(stderr(), "Fatal error: {}\n", e)).ok();
            set_exit_status(1);
        }
    }
}
