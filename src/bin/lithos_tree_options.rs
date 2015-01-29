use std::os::{getenv, args};
use std::io::stdio::{stdout, stderr};
use argparse::{ArgumentParser, Store};


pub struct Options {
    pub config_file: Path,
}

impl Options {
    pub fn parse_args() -> Result<Options, isize> {
        Options::parse_specific_args(args(), &mut stdout(), &mut stderr())
    }
    pub fn parse_specific_args(args: Vec<String>,
        stdout: &mut Writer, stderr: &mut Writer)
        -> Result<Options, isize>
    {
        let mut options = Options {
            config_file: Path::new("/etc/lithos.yaml"),
        };
        let parse_result = {
            let mut ap = ArgumentParser::new();
            ap.set_description("Runs tree of processes");
            ap.refer(&mut options.config_file)
              .add_option(&["-C", "--config"], Box::new(Store::<Path>),
                "Name of the global configuration file (default /etc/lithos.yaml)")
              .metavar("FILE");
            ap.parse(args, stdout, stderr)
        };
        match parse_result {
            Ok(()) => Ok(options),
            Err(x) => Err(x),
        }
    }
}
