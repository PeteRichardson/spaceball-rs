use comfy_table::{presets, Attribute, Cell, Color, Table};
use crossterm::style::Stylize;
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

struct PortEntry {
    label: &'static str,
    product: String,
    manufacturer: String,
    serial: String,
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
    let mut known: HashMap<String, PortEntry> = HashMap::new();

    // Initial scan — treat all ports present at startup as "just connected".
    for info in candidate_ports() {
        let label = probe_port(&info.port_name);
        let (product, manufacturer, serial) = usb_fields(&info);
        let secs = start.elapsed().as_secs();
        let product_col = format!("{:<20}", product);
        let mfg_col = format!("{:<20}", manufacturer);
        println!(
            "[{:>4}s] {}  {:<9}  {}  {}  {:<15}  {}",
            secs,
            "connected   ".green(),
            label,
            product_col.as_str().blue(),
            mfg_col.as_str().blue(),
            serial,
            info.port_name.as_str().yellow(),
        );
        known.insert(info.port_name, PortEntry { label, product, manufacturer, serial });
    }

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let current_ports = candidate_ports();
        let current_names: HashSet<String> =
            current_ports.iter().map(|i| i.port_name.clone()).collect();

        // New ports: probe, cache, and print.
        for info in &current_ports {
            if !known.contains_key(&info.port_name) {
                let label = probe_port(&info.port_name);
                let (product, manufacturer, serial) = usb_fields(info);
                let secs = start.elapsed().as_secs();
                let product_col = format!("{:<20}", product);
                let mfg_col = format!("{:<20}", manufacturer);
                println!(
                    "[{:>4}s] {}  {:<9}  {}  {}  {:<15}  {}",
                    secs,
                    "connected   ".green(),
                    label,
                    product_col.as_str().blue(),
                    mfg_col.as_str().blue(),
                    serial,
                    info.port_name.as_str().yellow(),
                );
                known.insert(
                    info.port_name.clone(),
                    PortEntry { label, product, manufacturer, serial },
                );
            }
        }

        // Removed ports: print cached metadata and drop.
        let secs = start.elapsed().as_secs();
        let gone: Vec<String> = known
            .keys()
            .filter(|p| !current_names.contains(*p))
            .cloned()
            .collect();
        for path in gone {
            let entry = known.remove(&path).unwrap();
            let product_col = format!("{:<20}", entry.product);
            let mfg_col = format!("{:<20}", entry.manufacturer);
            println!(
                "[{:>4}s] {}  {:<9}  {}  {}  {:<15}  {}",
                secs,
                "disconnected".red(),
                entry.label,
                product_col.as_str().blue(),
                mfg_col.as_str().blue(),
                entry.serial,
                path.as_str().yellow(),
            );
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
