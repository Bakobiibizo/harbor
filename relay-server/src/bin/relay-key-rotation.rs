use clap::Parser;
use ed25519_dalek::VerifyingKey;
use libp2p::identity::Keypair;
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

#[derive(Debug, Parser)]
#[command(about = "Create a Harbor relay-key successor record without exposing private keys")]
struct Args {
    #[arg(long)]
    current_key: PathBuf,
    #[arg(long)]
    relay: String,
    #[arg(long)]
    previous_key_id: String,
    #[arg(long)]
    next_key_id: String,
    #[arg(long)]
    next_key: PathBuf,
    #[arg(long)]
    sequence: u64,
    #[arg(long)]
    not_before: i64,
    #[arg(long)]
    not_after: i64,
    #[arg(long)]
    issued_at: Option<i64>,
    #[arg(long)]
    compromise_from: Option<i64>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct RelayKeyRotation {
    domain: String,
    version: u16,
    relay: String,
    previous_key_id: String,
    next_key_id: String,
    next_public_key: Vec<u8>,
    not_before: i64,
    not_after: i64,
    issued_at: i64,
    sequence: u64,
    compromise_from: Option<i64>,
}

#[derive(Debug, Serialize)]
struct SignedRelayKeyRotation {
    rotation: RelayKeyRotation,
    previous_key_signature: Vec<u8>,
}

fn load_or_generate_successor(path: &PathBuf) -> Result<Keypair, Box<dyn std::error::Error>> {
    if path.exists() {
        return Ok(Keypair::from_protobuf_encoding(&fs::read(path)?)?);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let key = Keypair::generate_ed25519();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&key.to_protobuf_encoding()?)?;
    Ok(key)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.sequence == 0
        || args.previous_key_id.is_empty()
        || args.next_key_id.is_empty()
        || args.previous_key_id == args.next_key_id
        || args.not_after <= args.not_before
    {
        return Err("invalid rotation sequence, key IDs, or validity window".into());
    }
    let successor = load_or_generate_successor(&args.next_key)?;
    let successor_ed = successor
        .public()
        .try_into_ed25519()
        .map_err(|_| "successor relay identity must be Ed25519")?;
    let next_public_key = successor_ed.to_bytes().to_vec();
    let next_raw: [u8; 32] = next_public_key
        .as_slice()
        .try_into()
        .map_err(|_| "successor Ed25519 public key must decode to exactly 32 bytes")?;
    VerifyingKey::from_bytes(&next_raw)?;

    let current = Keypair::from_protobuf_encoding(&fs::read(&args.current_key)?)?;
    current
        .clone()
        .try_into_ed25519()
        .map_err(|_| "current relay identity must be Ed25519")?;
    let rotation = RelayKeyRotation {
        domain: "harbor/relay-key-rotation/1".into(),
        version: 1,
        relay: args.relay,
        previous_key_id: args.previous_key_id,
        next_key_id: args.next_key_id,
        next_public_key,
        not_before: args.not_before,
        not_after: args.not_after,
        issued_at: args
            .issued_at
            .unwrap_or_else(|| chrono::Utc::now().timestamp()),
        sequence: args.sequence,
        compromise_from: args.compromise_from,
    };
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&rotation, &mut bytes)?;
    let signed = SignedRelayKeyRotation {
        previous_key_signature: current.sign(&bytes)?,
        rotation,
    };
    let output = serde_json::to_string_pretty(&signed)?;
    if let Some(path) = args.output {
        fs::write(path, format!("{output}\n"))?;
    } else {
        println!("{output}");
    }
    Ok(())
}
