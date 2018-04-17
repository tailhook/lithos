use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::fs::{File};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::str::from_utf8;

use base64;
use blake2::{Blake2b, digest::VariableOutput, digest::Input};
use failure::{Error, ResultExt};
use ssh_keys::{PrivateKey, openssh};

use lithos::nacl;
use lithos::sandbox_config::SandboxConfig;
use lithos::child_config::ChildInstance;


fn parse_private_key(filename: &Path) -> Result<Vec<PrivateKey>, Error> {
    let mut buf = String::with_capacity(1024);
    let mut f = File::open(filename)
        .context(Path::new(filename).display().to_string())?;
    let meta = f.metadata()
        .context(Path::new(filename).display().to_string())?;
    if meta.uid() != 0 {
        bail!("Key must be owned by root");
    }
    if meta.mode() & 0o777 & !0o600 != 0 {
        bail!("Key's mode must be 0600");
    }
    f.read_to_string(&mut buf)
        .context(Path::new(filename).display().to_string())?;
    Ok(openssh::parse_private_key(&buf)?)
}

fn b2_short_hash(data: &[u8]) -> String {
    let mut buf = [0u8; 6];
    let mut hash: Blake2b = VariableOutput::new(buf.len()).expect("blake2b");
    hash.process(data);
    hash.variable_result(&mut buf[..]).expect("blake2b");
    return base64::encode(&buf[..])
}

fn decrypt(key: &PrivateKey, namespaces: &HashSet<&str>, value: &str)
    -> Result<String, Error>
{
    let key_bytes = match *key {
        PrivateKey::Ed25519(key) => key,
        _ => bail!("Only ed25519 keys are supported"),
    };
    let (private_key, public_key) = key_bytes.split_at(32);
    if !value.starts_with("v2:") {
        bail!("Only v2 secrets are supported");
    }
    let mut it = value.split(":");
    it.next(); // skip version
    let (key_hash, ns_hash, secr_hash, cipher) = {
        match (it.next(), it.next(), it.next(), it.next(), it.next()) {
            (Some(key), Some(ns), Some(secr), Some(cipher), None) => {
                (key, ns, secr, base64::decode(cipher)?)
            }
            _ => bail!("invalid key format"),
        }
    };

    let plain = nacl::crypto_box_edwards_seal_open(
        &cipher, public_key, private_key)?;

    let mut pair = plain.splitn(2, |&x| x == b':');
    let namespace = from_utf8(pair.next().unwrap())
        .map_err(|_| format_err!("can't decode namespace from utf-8"))?;
    let secret = pair.next().ok_or(format_err!("decrypted data is invalid"))?;

    if b2_short_hash(public_key) != key_hash {
        bail!("invalid key hash");
    }
    if b2_short_hash(namespace.as_bytes()) != ns_hash {
        bail!("invalid namespace hash");
    }
    if b2_short_hash(&secret) != secr_hash {
        bail!("invalid secret hash");
    }
    if !namespaces.contains(namespace) {
        bail!("expected namespaces {:?} got {:?}", namespaces, namespace);
    }
    if secret.contains(&0) {
        bail!("no null bytes allowed in secret");
    }

    String::from_utf8(secret.to_vec())
        .map_err(|_| format_err!("Can't decode secret as utf-8"))
}

fn decrypt_pair(keys: &[PrivateKey], namespaces: &HashSet<&str>,
    values: &[String])
    -> Result<String, Vec<Error>>
{
    let mut errs = Vec::new();
    for key in keys {
        for value in values {
            match decrypt(key, namespaces, value) {
                Ok(value) => return Ok(value),
                Err(e) => errs.push(e),
            }
        }
    }
    Err(errs)
}

pub fn decode(sandbox: &SandboxConfig, child_config: &ChildInstance,
    secrets: &BTreeMap<String, Vec<String>>)
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

    let mut all_namespaces = HashSet::new();
    if sandbox.secrets_namespaces.len() == 0 {
        all_namespaces.insert("");
    } else {
        all_namespaces.extend(
            sandbox.secrets_namespaces.iter().map(|x| &x[..]))
    };
    all_namespaces.extend(
        child_config.extra_secrets_namespaces.iter().map(|x| &x[..]));

    let mut res = BTreeMap::new();

    for (name, values) in secrets {
        res.insert(name.clone(), decrypt_pair(&keys, &all_namespaces, values)
            .map_err(|e| {
                format_err!("Can't decrypt secret {:?}, errors: {}", name,
                    e.iter().map(|x| x.to_string())
                    .collect::<Vec<_>>().join(", "))
            })?);
    }

    Ok(res)
}
