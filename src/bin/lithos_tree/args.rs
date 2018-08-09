use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::thread::sleep;
use std::time::{Instant, Duration};

use nix::unistd::Pid;

pub enum Child {
    Normal { name: String, config: String },
    Zombie,
    Unidentified,
    Error,
}

pub fn read(pid: Pid, global_config: &Path) -> Child {
    use self::Child::*;
    let start = Instant::now();
    loop {
        let mut buf = String::with_capacity(4096);
        match File::open(&format!("/proc/{}/cmdline", pid))
             .and_then(|mut f| f.read_to_string(&mut buf))
        {
            Ok(_) => {},
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                // TODO(tailhook) actually already dead, but shouldn't happen
                return Zombie;
            }
            Err(e) => {
                warn!("Error opening /proc/{}/cmdline: {}", pid, e);
                return Error;
            }
        }
        let args: Vec<&str> = buf[..].splitn(8, '\0').collect();
        if args[0].len() == 0 {
            return Zombie;
        }

        if Path::new(args[0]).file_name()
          .and_then(|x| x.to_str()) == Some("lithos_tree")
        {
            if start + Duration::new(1, 0) > Instant::now() {
                sleep(Duration::from_millis(2));
                continue;
            } else {
                error!("Child did not exec'd in > 1 sec");
                return Error;
            }
        }

        if args.len() != 8
           || Path::new(args[0]).file_name()
              .and_then(|x| x.to_str()) != Some("lithos_knot")
           || args[1] != "--name"
           || args[3] != "--master"
           || Path::new(args[4]) != global_config
           || args[5] != "--config"
           || args[7] != ""
        {
            return Unidentified;
        }
        return Normal {
            name: args[2].to_string(),
            config: args[6].to_string(),
        };
    }
}
