use pretty_hex::*;
use spaceball_rs::Spaceball;

const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_PORT.to_string());

    eprintln!("Connecting to Spaceball at {path} ...");

    let mut sm = Spaceball::open(&path)?;

    eprintln!("Initialized. Reading bytes (Ctrl-C to quit):\n");

    let cfg = HexConfig {
        title: false,
        ..HexConfig::default()
    };

    loop {
        let mut packet = Vec::new();
        for b in sm.bytes() {
            match b {
                Ok(b'\r') => break,
                Ok(byte) => packet.push(byte),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(Box::new(e)),
            }
        }
        if !packet.is_empty() {
            println!("{:?}", packet.hex_conf(cfg));
        }
    }
}
