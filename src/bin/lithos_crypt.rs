extern crate base64;
extern crate rand;
extern crate lithos;
extern crate ssh_keys;
#[macro_use] extern crate failure;
#[macro_use] extern crate structopt;


use std::fs::File;
use std::io::{Read, BufReader, BufRead, Write, stdout, stderr};
use std::path::Path;
use std::process::exit;

use failure::{Error, ResultExt};
use rand::Rng;
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

fn encrypt(e: EncryptOpt) -> Result<(), Error> {
    let key_bytes = match e.key {
        PublicKey::Ed25519(key) => key,
        _ => bail!("Only ed25519 keys are supported"),
    };
    let mut nonce = [0u8; 24];
    let mut rng = rand::OsRng::new()?;
    rng.fill_bytes(&mut nonce);
    let cypher = nacl::crypto_secretbox(
        e.data.as_bytes(), &nonce[..], &key_bytes[..]);
    let mut buf = Vec::with_capacity(cypher.len() + 32 + 24);
    buf.write(&key_bytes[..]).unwrap();
    buf.write(&nonce[..]).unwrap();
    buf.write(&cypher).unwrap();
    let data = base64::encode(&buf);
    println!("v1:{}", data);
    Ok(())
}

fn decrypt(e: DecryptOpt) -> Result<(), Error> {
    let key_bytes = match e.key {
        PrivateKey::Ed25519(key) => key,
        _ => bail!("Only ed25519 keys are supported"),
    };
    if !e.data.starts_with("v1:") {
        bail!("Only v1 secrets are supported");
    }
    let data = base64::decode(&e.data["v1:".len()..])?;
    if data.len() < 32+24 {
        bail!("data is too short");
    }
    let plain = nacl::crypto_secretbox_open(
        &data[32+24..], &data[32..32+24], &key_bytes[32..])
        .map_err(|e| format_err!("{}", e))?;
    let mut out = stdout();
    out.write_all(&plain)?;
    out.flush()?;
    stderr().write_all(b"\n")?; // nicer in print
    Ok(())
}

fn main() {
    use Options::*;
    let opt = Options::from_args();
    let res = match opt {
        Encrypt(e) => encrypt(e),
        Decrypt(d) => decrypt(d),
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
