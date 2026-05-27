use spaceball_rs::{Probeable, Spaceball, SpaceOrb};
use std::collections::HashMap;
use std::time::Instant;

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
    todo!()
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
