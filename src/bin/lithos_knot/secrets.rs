use std::collections::BTreeMap;
use std::io::Read;
use std::fs::{File};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use base64;
use failure::{Error, ResultExt};
use ssh_keys::{PrivateKey, openssh};

use lithos::nacl;
use lithos::sandbox_config::SandboxConfig;


fn parse_private_key(filename: &Path) -> Result<Vec<PrivateKey>, Error> {
    let mut buf = String::with_capacity(1024);
    let f = File::open(filename)?
        .context(Path::new(filename).display().to_string())?;
    let meta = f.metadata()?
        .context(Path::new(filename).display().to_string())?;
    if f.uid() != 0 {
        bail!("Key must be owned by root");
    }
    if f.mode() & 0o777 & !0o600 != 0 {
        bail!("Key's mode must be 0600");
    }
    f.read_to_string(&mut buf)?;
        .context(Path::new(filename).display().to_string())?;
    Ok(openssh::parse_private_key(&buf)?)
}

fn decrypt(key: &PrivateKey, value: &str) -> Result<String, Error> {
    let key_bytes = match *key {
        PrivateKey::Ed25519(key) => key,
        _ => bail!("Only ed25519 keys are supported"),
    };
    if !value.starts_with("v1:") {
        bail!("Only v1 secrets are supported");
    }
    let data = base64::decode(&value["v1:".len()..])?;
    if data.len() < 32+24 {
        bail!("data is too short");
    }
    let plain = nacl::crypto_secretbox_open(
        &data[32+24..], &data[32..32+24], &key_bytes[32..])
        .map_err(|e| format_err!("{}", e))?;
    String::from_utf8(plain)
        .map_err(|_| format_err!("Can't decode secret as utf-8"))
}

fn decrypt_pair(keys: &[PrivateKey], values: &[String])
    -> Result<String, Vec<Error>>
{
    let mut errs = Vec::new();
    for key in keys {
        for value in values {
            match decrypt(key, value) {
                Ok(value) => return Ok(value),
                Err(e) => errs.push(e),
            }
        }
    }
    Err(errs)
}

pub fn decode(sandbox: &SandboxConfig, secrets: &BTreeMap<String, Vec<String>>)
    -> Result<BTreeMap<String, String>, Error>
{
    if secrets.len() == 0 {
        // do not read keys
        return Ok(BTreeMap::new());
    }

    let keys = if let Some(ref filename) = sandbox.secrets_private_key {
        parse_private_key(&filename)?
    } else {
        bail!("No secrets key file defined to decode secrets: {:?}",
            secrets.keys());
    };

    let mut res = BTreeMap::new();

    for (name, values) in secrets {
        res.insert(name.clone(), decrypt_pair(&keys, values)
            .map_err(|e| {
                format_err!("Can't decrypt secret {:?}, errors: {}", name,
                    e.iter().map(|x| x.to_string())
                    .collect::<Vec<_>>().join(", "))
            })?);
    }

    Ok(res)
}
