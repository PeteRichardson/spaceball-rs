use comfy_table::{presets, Attribute, Cell, Color, Table};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

fn candidate_ports() -> Vec<serialport::SerialPortInfo> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            !p.port_name.contains("/dev/tty.")
                && matches!(p.port_type, serialport::SerialPortType::UsbPort(_))
        })
        .collect()
}

fn probe_port(path: &str) -> &'static str {
    match spaceball_rs::probe(path) {
        Ok(device) => device.device_id(),
        Err(_) => "?",
    }
}

fn usb_fields(info: &serialport::SerialPortInfo) -> (String, String, String) {
    match &info.port_type {
        serialport::SerialPortType::UsbPort(u) => (
            u.product.as_deref().unwrap_or("?").to_owned(),
            u.manufacturer.as_deref().unwrap_or("?").to_owned(),
            u.serial_number.as_deref().unwrap_or("?").to_owned(),
        ),
        _ => ("?".into(), "?".into(), "?".into()),
    }
}

fn cmd_list() {
    let ports = candidate_ports();
    if ports.is_empty() {
        println!("(no serial ports found)");
        return;
    }
    let mut table = Table::new();
    table.load_preset(presets::NOTHING);
    let bold = |s| Cell::new(s).add_attribute(Attribute::Bold);
    table.set_header([bold("Device"), bold("Product"), bold("Manufacturer"), bold("Serial"), bold("Port")]);
    for info in &ports {
        let label = probe_port(&info.port_name);
        let (product, manufacturer, serial) = usb_fields(info);
        table.add_row([
            Cell::new(label),
            Cell::new(&product).fg(Color::Blue),
            Cell::new(&manufacturer).fg(Color::Blue),
            Cell::new(&serial),
            Cell::new(&info.port_name).fg(Color::Yellow),
        ]);
    }
    println!("{table}");
}

fn cmd_watch() {
    let start = Instant::now();
    let mut known: HashMap<String, &'static str> = HashMap::new();

    // Initial scan — treat all ports present at startup as "just connected".
    for info in candidate_ports() {
        let label = probe_port(&info.port_name);
        let secs = start.elapsed().as_secs();
        println!("[{:>4}s] + {:<9}  {}", secs, label, info.port_name);
        known.insert(info.port_name, label);
    }

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let current: HashSet<String> = candidate_ports().into_iter().map(|i| i.port_name).collect();

        // New ports: probe and add.
        for path in &current {
            if !known.contains_key(path) {
                let label = probe_port(path);
                let secs = start.elapsed().as_secs();
                println!("[{:>4}s] + {:<9}  {}", secs, label, path);
                known.insert(path.clone(), label);
            }
        }

        // Removed ports: print and drop.
        let secs = start.elapsed().as_secs();
        let gone: Vec<String> = known
            .keys()
            .filter(|p| !current.contains(*p))
            .cloned()
            .collect();
        for path in gone {
            let label = known.remove(&path).unwrap();
            println!("[{:>4}s] - {:<9}  {}", secs, label, path);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("list") => cmd_list(),
        Some("watch") => cmd_watch(),
        _ => {
            eprintln!("Usage: sbprobe <list|watch>");
            eprintln!("  list   Scan all serial ports, print device type, exit.");
            eprintln!("  watch  Print a line whenever a device connects or disconnects.");
            std::process::exit(1);
        }
    }
}
