use pretty_hex::*;
use spaceball_rs::{SpaceOrb, Spaceball};

const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let device = if args.iter().any(|a| a == "spaceorb") { "spaceorb" } else { "spaceball" };
    let path = args.iter()
        .find(|a| *a != "spaceball" && *a != "spaceorb")
        .cloned()
        .unwrap_or_else(|| DEFAULT_PORT.to_string());

    eprintln!("Connecting to {device} at {path} ...");

    let cfg = HexConfig { title: false, ..HexConfig::default() };

    if device == "spaceorb" {
        let mut orb = SpaceOrb::open(&path)?;
        eprintln!("Initialized. Reading bytes (Ctrl-C to quit):\n");
        dump_bytes(orb.bytes(), cfg);
    } else {
        let mut sb = Spaceball::open(&path)?;
        eprintln!("Initialized. Reading bytes (Ctrl-C to quit):\n");
        dump_bytes(sb.bytes(), cfg);
    }
    Ok(())
}

fn dump_bytes(
    bytes: impl Iterator<Item = Result<u8, std::io::Error>>,
    cfg: HexConfig,
) {
    let mut packet = Vec::new();
    for b in bytes {
        match b {
            Ok(b'\r') => {
                if !packet.is_empty() {
                    println!("{:?}", packet.hex_conf(cfg));
                    packet.clear();
                }
            }
            Ok(byte) => packet.push(byte),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if !packet.is_empty() {
                    println!("{:?}", packet.hex_conf(cfg));
                    packet.clear();
                }
            }
            Err(_) => break,
        }
    }
}
