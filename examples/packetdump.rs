use spaceball_rs::{SpaceOrb, SpaceOrbPacket, Spaceball, SpaceballPacket};

const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let device = if args.iter().any(|a| a == "spaceorb") { "spaceorb" } else { "spaceball" };
    let path = args.iter()
        .find(|a| *a != "spaceball" && *a != "spaceorb")
        .cloned()
        .unwrap_or_else(|| DEFAULT_PORT.to_string());

    eprintln!("Connecting to {device} at {path} ...\n");

    if device == "spaceorb" {
        let mut orb = SpaceOrb::open(&path)?;
        for packet in orb.packets() {
            match packet? {
                SpaceOrbPacket::Ball(b) => {
                    let [fx, fy, fz] = b.force;
                    let [tx, ty, tz] = b.torque;
                    println!("BALL  F({fx:6},{fy:6},{fz:6})  T({tx:6},{ty:6},{tz:6})");
                }
                SpaceOrbPacket::Key(k) => {
                    let btns: String = ['A', 'B', 'C', 'D', 'E', 'F']
                        .iter()
                        .zip(k.buttons.iter())
                        .filter(|(_, p)| **p)
                        .map(|(c, _)| *c)
                        .collect();
                    println!(
                        "KEY   rezero={:<5} buttons=[{}]",
                        k.rezero,
                        if btns.is_empty() { "none".into() } else { btns }
                    );
                }
                SpaceOrbPacket::Reset(s) => println!("RESET {s}"),
                SpaceOrbPacket::Error { brown_out, eeprom, hardware } => {
                    println!("ERR   brown_out={brown_out} eeprom={eeprom} hw={hardware}");
                }
                SpaceOrbPacket::Unknown(raw) => {
                    print!("UNK  ");
                    for b in &raw { print!(" {b:02x}"); }
                    println!();
                }
            }
        }
    } else {
        let mut sb = Spaceball::open(&path)?;
        for packet in sb.packets() {
            match packet? {
                SpaceballPacket::Ball(b) => {
                    let [tx, ty, tz] = b.translation;
                    let [rx, ry, rz] = b.rotation;
                    println!(
                        "BALL  period={:5}  T({tx:6},{ty:6},{tz:6})  R({rx:6},{ry:6},{rz:6})",
                        b.period
                    );
                }
                SpaceballPacket::Key(k) => {
                    let btns: String = k
                        .buttons
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| **p)
                        .map(|(i, _)| format!("{}", i + 1))
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!(
                        "KEY   pick={:<5} buttons=[{}]",
                        k.pick,
                        if btns.is_empty() { "none".into() } else { btns }
                    );
                }
                SpaceballPacket::Unknown(raw) => {
                    print!("UNK  ");
                    for b in &raw { print!(" {b:02x}"); }
                    println!();
                }
            }
        }
    }
    Ok(())
}
