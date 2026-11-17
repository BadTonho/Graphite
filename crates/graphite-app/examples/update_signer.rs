//! Release signing tool for Graphite updates.
//!
//! ```text
//! cargo run -p graphite-app --example update_signer -- keygen <private-key-file>
//! cargo run -p graphite-app --example update_signer -- sign <private-key-file> <tag> <asset-file>
//! ```
//!
//! `keygen` writes a base64 PKCS#8 private key (keep it secret and never commit it) and prints
//! the public key as a Rust byte array to paste into `updates/signature.rs`.
//! `sign` writes `<asset>.sig` and `<asset>.sha256` next to the asset; upload both with it.
//! The signed message format must match `updates::signature::signed_message`.

use base64::Engine;
use ring::digest::{Context, SHA256};
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::io::Read;
use std::path::Path;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["keygen", key_file] => keygen(Path::new(key_file)),
        ["sign", key_file, tag, asset] => sign(Path::new(key_file), tag, Path::new(asset)),
        _ => Err("usage: update_signer keygen <key-file> | sign <key-file> <tag> <asset>".into()),
    }
}

fn keygen(key_file: &Path) -> Result<(), String> {
    if key_file.exists() {
        return Err(format!(
            "{} already exists; refusing to overwrite",
            key_file.display()
        ));
    }
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
        .map_err(|_| "key generation failed".to_owned())?;
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).map_err(|e| e.to_string())?;
    std::fs::write(key_file, B64.encode(pkcs8.as_ref())).map_err(|e| e.to_string())?;
    println!("Private key written to {}", key_file.display());
    println!("Public key (paste into TRUSTED_UPDATE_KEYS):");
    println!("{:?}", pair.public_key().as_ref());
    Ok(())
}

fn sign(key_file: &Path, tag: &str, asset: &Path) -> Result<(), String> {
    let encoded = std::fs::read_to_string(key_file).map_err(|e| e.to_string())?;
    let pkcs8 = B64.decode(encoded.trim()).map_err(|e| e.to_string())?;
    let pair = Ed25519KeyPair::from_pkcs8(&pkcs8).map_err(|e| e.to_string())?;

    let name = asset
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("asset has no file name")?;
    let mut file = std::fs::File::open(asset).map_err(|e| e.to_string())?;
    let mut ctx = Context::new(&SHA256);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    let sha: String = ctx
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let message = format!("graphite-update-v1\n{tag}\n{name}\n{sha}\n");
    let signature = B64.encode(pair.sign(message.as_bytes()).as_ref());

    let parent = asset.parent().unwrap_or(Path::new("."));
    std::fs::write(parent.join(format!("{name}.sig")), format!("{signature}\n"))
        .map_err(|e| e.to_string())?;
    std::fs::write(
        parent.join(format!("{name}.sha256")),
        format!("{sha}  {name}\n"),
    )
    .map_err(|e| e.to_string())?;
    println!("Signed {name} ({tag}); sha256 {sha}");
    Ok(())
}
