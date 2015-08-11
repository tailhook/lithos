use log;
use std::env;
use std::path::PathBuf;
use std::io::{Write, stdout, stderr};
use argparse::{ArgumentParser, Parse, StoreOption, StoreTrue};


pub struct Options {
    pub config_file: PathBuf,
    pub log_stderr: bool,
    pub log_level: Option<log::LogLevel>,
}

impl Options {
    pub fn parse_args() -> Result<Options, i32> {
        Options::parse_specific_args(env::args().collect(),
                                     &mut stdout(), &mut stderr())
    }
    pub fn parse_specific_args(args: Vec<String>,
        stdout: &mut Write, stderr: &mut Write)
        -> Result<Options, i32>
    {
        let mut options = Options {
            config_file: PathBuf::from("/etc/lithos/master.yaml"),
            log_stderr: false,
            log_level: None,
        };
        let parse_result = {
            let mut ap = ArgumentParser::new();
            ap.set_description("Runs tree of processes");
            ap.refer(&mut options.config_file)
              .add_option(&["-C", "--config"], Parse,
                "Name of the global configuration file \
                 (default /etc/lithos/master.yaml)")
              .metavar("FILE");
            ap.refer(&mut options.log_stderr)
              .add_option(&["--log-stderr"], StoreTrue,
                "Print debugging info to stderr");
            ap.refer(&mut options.log_level)
              .add_option(&["--log-level"], StoreOption,
                "Set log level (default info for now)");
            ap.parse(args, stdout, stderr)
        };
        match parse_result {
            Ok(()) => Ok(options),
            Err(x) => Err(x),
        }
    }
}
