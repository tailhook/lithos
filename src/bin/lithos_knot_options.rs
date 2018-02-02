use std::collections::BTreeMap;
use std::env;
use std::io::{stdout, stderr};
use std::io::{Write};
use std::path::{PathBuf};

use log;
use argparse::{ArgumentParser, StoreOption, Store, Parse, List, StoreTrue};
use argparse::{Print};

use lithos::child_config::ChildConfig;
use lithos::child_config::ChildKind::Daemon;


pub struct Options {
    pub master_config: PathBuf,
    pub config: ChildConfig,
    pub name: String,
    pub args: Vec<String>,
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
            master_config: PathBuf::from("/etc/lithos/master.yaml"),
            config: ChildConfig {
                instances: 0,
                image: "".to_string(),
                config: "".to_string(),
                variables: BTreeMap::new(),
                kind: Daemon,
            },
            name: "".to_string(),
            args: vec!(),
            log_stderr: false,
            log_level: None,
        };
        let parse_result = {
            let mut ap = ArgumentParser::new();
            ap.set_description("Runs tree of processes");
            ap.refer(&mut options.name)
              .add_option(&["--name"], Store,
                "The process name");
            ap.refer(&mut options.master_config)
              .add_option(&["--master"], Parse,
                "Name of the master configuration file \
                 (default /etc/lithos/master.yaml)")
              .metavar("FILE");
            ap.refer(&mut options.config)
              .add_option(&["--config"], Store,
                "JSON-serialized container configuration")
              .required()
              .metavar("JSON");
            ap.refer(&mut options.args)
              .add_argument("argument", List,
                "Additional arguments for the command");
            ap.refer(&mut options.log_stderr)
              .add_option(&["--log-stderr"], StoreTrue,
                "Print debugging info to stderr");
            ap.refer(&mut options.log_level)
              .add_option(&["--log-level"], StoreOption,
                "Set log level (default info for now)");
            ap.add_option(&["--version"],
                Print(env!("CARGO_PKG_VERSION").to_string()),
                "Show version");
            ap.stop_on_first_argument(true);
            ap.parse(args, stdout, stderr)
        };
        match parse_result {
            Ok(()) => Ok(options),
            Err(x) => Err(x),
        }
    }
}
