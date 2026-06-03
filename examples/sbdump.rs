use spaceball_rs::{
    DeviceEvent, NormalizedMotion, RawPacket,
    Spaceball, SpaceballPacket,
    SpaceOrb, SpaceOrbPacket,
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

fn run(device: &mut Device, args: &Args, start: Instant) {
    // Implemented in Tasks 8 and 9
    let _ = (device, args, start);
    todo!()
}
