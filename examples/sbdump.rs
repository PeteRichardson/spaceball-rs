use spaceball_rs::{
    DeviceEvent, NormalizedMotion, RawPacket,
    Spaceball, SpaceballBallEvent, SpaceballKeyEvent, SpaceballPacket,
    SpaceOrb, SpaceOrbBallEvent, SpaceOrbKeyEvent, SpaceOrbPacket,
    Probeable, SixDofDevice,
    ButtonState,
};
use std::time::Instant;

struct Args {
    path: Option<String>,
    force_spaceorb: bool,
    force_spaceball: bool,
    hex: bool,
    events: bool,
}

fn parse_args() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut args = Args {
        path: None,
        force_spaceorb: false,
        force_spaceball: false,
        hex: false,
        events: false,
    };
    for arg in &raw {
        match arg.as_str() {
            "--spaceorb"       => args.force_spaceorb = true,
            "--spaceball"      => args.force_spaceball = true,
            "--hex"            => args.hex = true,
            "-e" | "--events"  => args.events = true,
            other if other.starts_with("--") || other.starts_with('-') => {
                return Err(format!("unknown option: {other}"));
            }
            other => {
                if args.path.is_some() {
                    return Err("too many path arguments".into());
                }
                args.path = Some(other.to_string());
            }
        }
    }
    if args.force_spaceorb && args.force_spaceball {
        return Err("--spaceorb and --spaceball are mutually exclusive".into());
    }
    Ok(args)
}

fn candidate_ports() -> Vec<serialport::SerialPortInfo> {
    // TODO: expose this from lib.rs so examples don't duplicate it
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            !p.port_name.contains("/dev/tty.")
                && matches!(p.port_type, serialport::SerialPortType::UsbPort(_))
        })
        .collect()
}

enum Device {
    Orb(SpaceOrb),
    Ball(Spaceball),
}

impl Device {
    fn device_id(&self) -> &'static str {
        match self {
            Device::Orb(d)  => d.device_id(),
            Device::Ball(d) => d.device_id(),
        }
    }
}

fn open_device(args: &Args) -> Result<Device, String> {
    if args.force_spaceorb {
        let path = args.path.as_deref()
            .ok_or_else(|| "--spaceorb requires a PATH".to_string())?;
        return SpaceOrb::open(path)
            .map(Device::Orb)
            .map_err(|e| e.to_string());
    }
    if args.force_spaceball {
        let path = args.path.as_deref()
            .ok_or_else(|| "--spaceball requires a PATH".to_string())?;
        return Spaceball::open(path)
            .map(Device::Ball)
            .map_err(|e| e.to_string());
    }

    if let Some(path) = args.path.as_deref() {
        if let Ok(orb) = SpaceOrb::probe(path) {
            return Ok(Device::Orb(orb));
        }
        if let Ok(sb) = Spaceball::probe(path) {
            return Ok(Device::Ball(sb));
        }
        return Err(format!("no device found at {path}"));
    }

    let ports = candidate_ports();
    if ports.is_empty() {
        return Err("no serial ports found".into());
    }
    for info in &ports {
        if let Ok(orb) = SpaceOrb::probe(&info.port_name) {
            return Ok(Device::Orb(orb));
        }
        if let Ok(sb) = Spaceball::probe(&info.port_name) {
            return Ok(Device::Ball(sb));
        }
    }
    Err(format!("no supported device found (tried {} port(s))", ports.len()))
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("Usage: sbdump [PATH] [--spaceorb | --spaceball] [--hex] [-e | --events]");
            std::process::exit(1);
        }
    };

    let mut device = match open_device(&args) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let start = Instant::now();
    eprintln!("Connected: {} (Ctrl-C to quit)", device.device_id());
    run(&mut device, &args, start);
}

/// Format elapsed seconds as "   1.234s" (8 chars + trailing 's').
fn fmt_elapsed(start: Instant) -> String {
    format!("{:8.3}s", start.elapsed().as_secs_f64())
}

/// Format up to 20 raw bytes as a fixed-width hex column.
///
/// Bytes are grouped in fours with double spaces between groups.
/// Shorter packets are padded with spaces to maintain a constant 63-char width.
fn fmt_hex_col(bytes: &[u8]) -> String {
    const MAX: usize = 20;
    const GROUP: usize = 4;
    let mut out = String::with_capacity(63);
    for i in 0..MAX {
        if i > 0 {
            if i % GROUP == 0 { out.push_str("  "); } else { out.push(' '); }
        }
        if i < bytes.len() {
            out.push_str(&format!("{:02x}", bytes[i]));
        } else {
            out.push_str("  "); // padding for missing byte
        }
    }
    out
}

/// Print one dump line to stdout.
fn print_line(start: Instant, device_id: &str, raw_opt: Option<&[u8]>, parsed: &str) {
    let t   = fmt_elapsed(start);
    let dev = format!("{:<9}", device_id);
    match raw_opt {
        Some(raw) => println!("{t}  {dev}  {}  {parsed}", fmt_hex_col(raw)),
        None      => println!("{t}  {dev}  {parsed}"),
    }
}

// ── Packet-mode formatters ────────────────────────────────────────────────────

fn fmt_sb_ball(b: &SpaceballBallEvent) -> String {
    let [tx, ty, tz] = b.translation;
    let [rx, ry, rz] = b.rotation;
    format!("BALL  period={:5}  Tr({:6},{:6},{:6})  R({:6},{:6},{:6})",
        b.period, tx, ty, tz, rx, ry, rz)
}

fn fmt_sb_key(k: &SpaceballKeyEvent) -> String {
    let btns: String = k.buttons.iter()
        .enumerate()
        .filter(|&(_, &p)| p)
        .map(|(i, _)| (i + 1).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("KEY   pick={:<5}  buttons=[{}]",
        k.pick, if btns.is_empty() { "none".into() } else { btns })
}

fn fmt_orb_ball(b: &SpaceOrbBallEvent) -> String {
    let [fx, fy, fz] = b.force;
    let [tx, ty, tz] = b.torque;
    format!("BALL  F({:6},{:6},{:6})  Tq({:6},{:6},{:6})",
        fx, fy, fz, tx, ty, tz)
}

fn fmt_orb_key(k: &SpaceOrbKeyEvent) -> String {
    let btns: String = ['A', 'B', 'C', 'D', 'E', 'F']
        .iter()
        .zip(k.buttons.iter())
        .filter(|&(_, &p)| p)
        .map(|(c, _)| c.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    format!("KEY   rezero={:<5}  [{}]",
        k.rezero, if btns.is_empty() { "none".into() } else { btns })
}

fn fmt_unk(bytes: &[u8]) -> String {
    let hex = bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ");
    format!("UNK   {hex}")
}

fn run(device: &mut Device, args: &Args, start: Instant) {
    if args.events {
        run_events(device, args, start);
    } else {
        run_packets(device, args, start);
    }
}

fn run_packets(device: &mut Device, args: &Args, start: Instant) {
    match device {
        Device::Ball(sb) => run_packets_spaceball(sb, args, start),
        Device::Orb(orb) => run_packets_spaceorb(orb, args, start),
    }
}

fn run_packets_spaceball(sb: &mut Spaceball, args: &Args, start: Instant) {
    let id = "Spaceball";
    for result in sb.packets_with_bytes() {
        match result {
            Err(e) => eprintln!("error: {e}"),
            Ok(RawPacket { ref raw, ref packet }) => {
                let hex = if args.hex { Some(raw.as_slice()) } else { None };
                let parsed = match packet {
                    SpaceballPacket::Ball(b)    => fmt_sb_ball(b),
                    SpaceballPacket::Key(k)     => fmt_sb_key(k),
                    SpaceballPacket::Unknown(u) => fmt_unk(u),
                };
                print_line(start, id, hex, &parsed);
            }
        }
    }
}

fn run_packets_spaceorb(orb: &mut SpaceOrb, args: &Args, start: Instant) {
    let id = "SpaceOrb";
    for result in orb.packets_with_bytes() {
        match result {
            Err(e) => eprintln!("error: {e}"),
            Ok(RawPacket { ref raw, ref packet }) => {
                let hex = if args.hex { Some(raw.as_slice()) } else { None };
                let parsed = match packet {
                    SpaceOrbPacket::Ball(b)    => fmt_orb_ball(b),
                    SpaceOrbPacket::Key(k)     => fmt_orb_key(k),
                    SpaceOrbPacket::Reset(s)   => format!("RESET {s}"),
                    SpaceOrbPacket::Error { brown_out, eeprom, hardware } =>
                        format!("ERR   brown_out={brown_out}  eeprom={eeprom}  hw={hardware}"),
                    SpaceOrbPacket::Unknown(u) => fmt_unk(u),
                };
                print_line(start, id, hex, &parsed);
            }
        }
    }
}

fn run_events(_device: &mut Device, _args: &Args, _start: Instant) {
    todo!("events mode — implemented in Task 9")
}
