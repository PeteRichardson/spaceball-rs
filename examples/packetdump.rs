use spaceball_rs::{SpaceballPacket, Spaceball};

const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_PORT.to_string());

    eprintln!("Connecting to Spaceball at {path} ...");

    let mut sm = Spaceball::open(&path)?;

    eprintln!("Initialized. Reading packets (Ctrl-C to quit):\n");

    for packet in sm.packets() {
        match packet? {
            SpaceballPacket::Key(k) => {
                let btns: String = k
                    .buttons
                    .iter()
                    .enumerate()
                    .filter(|(_, pressed)| **pressed)
                    .map(|(i, _)| format!("{}", i + 1))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "KEY   pick={:<5} buttons=[{}]",
                    k.pick,
                    if btns.is_empty() {
                        "none".to_string()
                    } else {
                        btns
                    }
                );
            }
            SpaceballPacket::Ball(b) => {
                let [tx, ty, tz] = b.translation;
                let [rx, ry, rz] = b.rotation;
                println!(
                    "BALL  period={:5}  T({:6}, {:6}, {:6})  R({:6}, {:6}, {:6})",
                    b.period, tx, ty, tz, rx, ry, rz
                );
            }
            SpaceballPacket::Unknown(raw) => {
                print!("UNK  ");
                for byte in &raw {
                    print!(" {:02x}", byte);
                }
                println!();
            }
        }
    }

    Ok(())
}
