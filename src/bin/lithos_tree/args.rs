use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::thread::sleep;
use std::time::{Instant, Duration};

use nix::unistd::Pid;


fn discard<E>(_: E) { }

pub fn read(pid: Pid, global_config: &Path)
    -> Result<(String, String), ()>
{
    let start = Instant::now();
    loop {
        let mut buf = String::with_capacity(4096);
        File::open(&format!("/proc/{}/cmdline", pid))
             .and_then(|mut f| f.read_to_string(&mut buf))
             .map_err(discard)?;
        let args: Vec<&str> = buf[..].splitn(8, '\0').collect();

        if Path::new(args[0]).file_name()
          .and_then(|x| x.to_str()) == Some("lithos_tree")
        {
            if start + Duration::new(1, 0) > Instant::now() {
                sleep(Duration::from_millis(2));
                continue;
            } else {
                error!("Child did not exec'd in > 1 sec");
                return Err(());
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
            return Err(());
        }
        return Ok((args[2].to_string(), args[6].to_string()));
    }
}
