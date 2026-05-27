use spaceball_rs::{Probeable, Spaceball, SpaceOrb};
use std::collections::HashMap;
use std::time::Instant;

fn probe_port(path: &str) -> &'static str {
    todo!()
}

fn cmd_list() {
    todo!()
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
