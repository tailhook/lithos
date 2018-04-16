extern crate base64;
extern crate blake2;
extern crate rand;
extern crate regex;
extern crate lithos;
extern crate ssh_keys;
extern crate sha2;
extern crate crypto;
#[macro_use] extern crate failure;
#[macro_use] extern crate structopt;


use std::fs::File;
use std::io::{Read, BufReader, BufRead, Write, stdout, stderr};
use std::path::Path;
use std::process::exit;

use blake2::{Blake2b, digest::VariableOutput, digest::Input};
use failure::{Error, ResultExt};
use regex::Regex;
use ssh_keys::{PublicKey, PrivateKey, openssh};
use structopt::StructOpt;

use lithos::nacl;


#[derive(Debug, StructOpt)]
#[structopt(name = "lithos_crypt",
            about = "An utility to encrypt secrets for lithos. \
                     It also allows to decrypt secrets for introspection if \
                     you have access for private key (only for debugging)")]
enum Options {
    #[structopt(name="encrypt")]
    Encrypt(EncryptOpt),
    #[structopt(name="decrypt")]
    Decrypt(DecryptOpt),
    #[structopt(name="check-key")]
    CheckKey(CheckKeyOpt),
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Encrypt secret value to put in config")]
pub struct EncryptOpt {
    #[structopt(long="key-file", short="k", help="
        A openssh-formatted ed25519 public key to use for encryption
    ", parse(try_from_str="parse_public_key"))]
    key: PublicKey,
    #[structopt(long="data", short="d", help="data to encrypt")]
    data: String,
    #[structopt(long="namespace", short="n", help="
        secrets namespace. Only processes \"authorized\" to read this namespace
        will be able do decrypt the data.
    ", default_value="", parse(try_from_str="validate_namespace"))]
    namespace: String,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Check that the secret value is encrypted \
                     with specified public key")]
pub struct CheckKeyOpt {
    #[structopt(long="key-file", short="k", help="
        A openssh-formatted ed25519 public key to use for encryption
    ", parse(try_from_str="parse_public_key"))]
    key: PublicKey,
    #[structopt(long="data", short="d", help="data to encrypt")]
    data: String,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Decrypt secret value from config")]
pub struct DecryptOpt {
    #[structopt(long="key-file", short="i", help="
        A openssh-formatted ed25519 private key to use for decryption
    ", parse(try_from_str="parse_private_key"))]
    key: PrivateKey,
    #[structopt(long="data", short="d", help="base64-encoded data to decrypt")]
    data: String,
}

fn validate_namespace(namespace: &str) -> Result<String, Error> {
    if !Regex::new("^[a-zA-Z0-9_.-]*$").expect("valid re").is_match(namespace) {
        bail!("invalid namespace, \
            valid on should match regex `^[a-zA-Z0-9_.-]*$`");
    }
    Ok(namespace.to_string())
}

fn parse_public_key(filename: &str) -> Result<PublicKey, Error> {
    let mut buf = String::with_capacity(1024);
    File::open(filename)
        .and_then(|f| BufReader::new(f).read_line(&mut buf))
        .context(Path::new(filename).display().to_string())?;
    let key = openssh::parse_public_key(&buf)?;
    Ok(key)
}

fn parse_private_key(filename: &str) -> Result<PrivateKey, Error> {
    let mut buf = String::with_capacity(1024);
    File::open(filename)
        .and_then(|mut f| f.read_to_string(&mut buf))
        .context(Path::new(filename).display().to_string())?;
    let mut key = openssh::parse_private_key(&buf)?;
    Ok(key.pop().expect("at least one key parsed"))
}

fn b2_short_hash(data: &[u8]) -> String {
    let mut buf = [0u8; 6];
    let mut hash: Blake2b = VariableOutput::new(buf.len()).expect("blake2b");
    hash.process(data);
    hash.variable_result(&mut buf[..]).expect("blake2b");
    return base64::encode(&buf[..])
}

fn encrypt(e: EncryptOpt) -> Result<(), Error> {
    let key_bytes = match e.key {
        PublicKey::Ed25519(key) => key,
        _ => bail!("Only ed25519 keys are supported"),
    };
    let plaintext = format!("{}:{}", e.namespace, e.data);
    let cypher = nacl::crypto_box_edwards_seal(
        plaintext.as_bytes(), &key_bytes[..]);
    let mut buf = Vec::with_capacity(cypher.len() + 24);
    buf.write(&cypher).unwrap();
    let data = base64::encode(&buf);
    println!("v2:{}:{}:{}:{}",
        b2_short_hash(&key_bytes[..]),
        b2_short_hash(e.namespace.as_bytes()),
        b2_short_hash(e.data.as_bytes()),
        data);
    Ok(())
}

fn check_key(o: CheckKeyOpt) -> Result<(), Error> {
    let key_bytes = match o.key {
        PublicKey::Ed25519(key) => key,
        _ => bail!("Only ed25519 keys are supported"),
    };
    if !o.data.starts_with("v1:") {
        bail!("Only v1 secrets are supported");
    }
    let data = base64::decode(&o.data["v1:".len()..])?;
    if data.len() < 32+24 {
        bail!("data is too short");
    }
    if data[..32] != key_bytes {
        bail!("Key mismatch");
    }
    Ok(())
}

fn decrypt(e: DecryptOpt) -> Result<(), Error> {
    let key_bytes = match e.key {
        PrivateKey::Ed25519(key) => key,
        _ => bail!("Only ed25519 keys are supported"),
    };
    if !e.data.starts_with("v2:") {
        bail!("Only v2 secrets are supported");
    }
    let mut it = e.data.split(":");
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
        &cipher, &key_bytes[32..], &key_bytes[..32])
        .map_err(|e| format_err!("Decryption error: {}", e))?;
    let mut pair = plain.splitn(2, |&x| x == b':');
    let namespace = pair.next().unwrap();
    let secret = pair.next().ok_or(format_err!("decrypted data is invalid"))?;

    if b2_short_hash(&key_bytes[32..]) != key_hash {
        bail!("invalid key hash");
    }
    if b2_short_hash(&namespace) != ns_hash {
        bail!("invalid namespace hash");
    }
    if b2_short_hash(&secret) != secr_hash {
        bail!("invalid secret hash");
    }

    let mut err = stderr();
    err.write_all(&namespace)?;
    err.write_all(b":")?;
    err.flush()?;
    let mut out = stdout();
    out.write_all(&secret)?;
    out.flush()?;
    err.write_all(b"\n")?; // nicer in print
    Ok(())
}

fn main() {
    use Options::*;
    let opt = Options::from_args();
    let res = match opt {
        Encrypt(e) => encrypt(e),
        Decrypt(d) => decrypt(d),
        CheckKey(c) => check_key(c),
    };
    match res {
        Ok(()) => {
            exit(0);
        }
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    }
}
