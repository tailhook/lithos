#[macro_use] extern crate log;
extern crate env_logger;
extern crate argparse;
extern crate quire;
extern crate lithos;

use std::env;
use std::path::PathBuf;
use std::process::exit;

use quire::parse_config;
use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue};
use lithos::master_config::MasterConfig;


fn main() {

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();
    let mut config_file = PathBuf::from("/etc/lithos.yaml");
    let mut verbose = false;
    let mut ver_min = 0;
    let mut ver_max = 1000;
    let mut days = None::<u32>;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Show used/unused images and clean if needed");
        ap.refer(&mut config_file)
          .add_option(&["-C", "--config"], Parse,
            "Name of the global configuration file (default /etc/lithos.yaml)")
          .metavar("FILE");
        ap.refer(&mut days)
          .add_option(&["-D", "--history-days"], ParseOption,
            r"Keep images that used no more than DAYS ago.
              There is no reasonable default, so you should specify this
              argument or --versions-min to get sane behavior.")
          .metavar("DAYS");
        ap.refer(&mut ver_min)
          .add_option(&["--vmin", "--versions-min"], Parse,
            r"Keep minimum NUM versions (even if they are older than DAYS).
              Default is 0 which means keep only current version.")
          .metavar("NUM");
        ap.refer(&mut ver_max)
          .add_option(&["--vmax", "--versions-max"], Parse,
            r"Keep maximum NUM versions
              (even if need to delete images more recent than DAYS).
              Default is 1000.")
          .metavar("NUM");
        ap.refer(&mut verbose)
          .add_option(&["-v", "--verbose"], StoreTrue,
            "Verbose output");
        ap.parse_args_or_exit();
    }
    let master: MasterConfig = match parse_config(&config_file,
        &*MasterConfig::validator(), Default::default()) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Can't parse config: {}", e);
            exit(1);
        }
    };
}
