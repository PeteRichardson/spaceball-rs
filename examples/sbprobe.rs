use spaceball_rs::{Probeable, Spaceball, SpaceOrb};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

fn probe_port(path: &str) -> &'static str {
    if SpaceOrb::probe(path).is_ok() {
        return "SpaceOrb";
    }
    if Spaceball::probe(path).is_ok() {
        return "Spaceball";
    }
    "?"
}

fn cmd_list() {
    let ports = serialport::available_ports().unwrap_or_default();
    if ports.is_empty() {
        println!("(no serial ports found)");
        return;
    }
    for info in &ports {
        let label = probe_port(&info.port_name);
        println!("{:<9}  {}", label, info.port_name);
    }
}

fn cmd_watch() {
    let start = Instant::now();
    let mut known: HashMap<String, &'static str> = HashMap::new();

    // Initial scan — treat all ports present at startup as "just connected".
    for info in serialport::available_ports().unwrap_or_default() {
        let label = probe_port(&info.port_name);
        let secs = start.elapsed().as_secs();
        println!("[{:>4}s] + {:<9}  {}", secs, label, info.port_name);
        known.insert(info.port_name, label);
    }

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let current: HashSet<String> = serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|info| info.port_name)
            .collect();

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
        let gone: Vec<String> = known.keys()
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
        Some("list")  => cmd_list(),
        Some("watch") => cmd_watch(),
        _ => {
            eprintln!("Usage: sbprobe <list|watch>");
            eprintln!("  list   Scan all serial ports, print device type, exit.");
            eprintln!("  watch  Print a line whenever a device connects or disconnects.");
            std::process::exit(1);
        }
    }
}
