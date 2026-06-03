# sbprobe cmd_watch Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add USB metadata columns (product, manufacturer, serial) and crossterm colors to `cmd_watch`, with "connected"/"disconnected" event words in green/red, matching the visual style of `cmd_list`.

**Architecture:** Extract a `usb_fields()` helper to eliminate duplicated USB field extraction, add a `PortEntry` struct to cache metadata per port for use on detach, restructure the watch loop to retain full `SerialPortInfo`, and apply crossterm `.Stylize` colors using a pad-then-color pattern throughout.

**Tech Stack:** Rust, `crossterm 0.29` (Stylize trait for terminal colors), `serialport 4` (SerialPortInfo/UsbPortInfo structs)

---

## Files

- Modify: `Cargo.toml` — add `crossterm = "0.29"` to `[dev-dependencies]`
- Modify: `examples/sbprobe.rs` — all code changes

---

### Task 1: Add crossterm to dev-dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add crossterm to `[dev-dependencies]`**

In `Cargo.toml`, add `crossterm = "0.29"` immediately after `comfy-table = "7"`:

```toml
[dev-dependencies]
comfy-table = "7"
crossterm = "0.29"
three-d = { version = "0.19.0", features = ["egui-gui"] }
bevy = "0.18.1"
bevy_rapier3d = "0.34.0"
rand = { version = "0.10.1" }
```

- [ ] **Step 2: Verify it resolves**

```bash
cargo check --example sbprobe
```

Expected: compiles clean (crossterm is already in the lockfile as a transitive dep, so no download needed).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(sbprobe): add crossterm as explicit dev-dependency"
```

---

### Task 2: Extract `usb_fields()` and simplify `cmd_list`

**Files:**
- Modify: `examples/sbprobe.rs`

- [ ] **Step 1: Add `usb_fields()` after `probe_port()`**

Insert this function between `probe_port` and `cmd_list` in `examples/sbprobe.rs`:

```rust
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
```

- [ ] **Step 2: Replace the inline match block in `cmd_list` with a call to `usb_fields()`**

The current `cmd_list` loop body:

```rust
    for info in &ports {
        let label = probe_port(&info.port_name);
        let (product, manufacturer, serial) = match &info.port_type {
            serialport::SerialPortType::UsbPort(u) => (
                u.product.as_deref().unwrap_or("?"),
                u.manufacturer.as_deref().unwrap_or("?"),
                u.serial_number.as_deref().unwrap_or("?"),
            ),
            _ => ("?", "?", "?"),
        };
        table.add_row([
            Cell::new(label),
            Cell::new(product).fg(Color::Blue),
            Cell::new(manufacturer).fg(Color::Blue),
            Cell::new(serial),
            Cell::new(&info.port_name).fg(Color::Yellow),
        ]);
    }
```

Replace with:

```rust
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
```

- [ ] **Step 3: Verify**

```bash
cargo check --example sbprobe
```

Expected: compiles clean. No behavioral change to `cmd_list`.

- [ ] **Step 4: Commit**

```bash
git add examples/sbprobe.rs
git commit -m "refactor(sbprobe): extract usb_fields() helper, use in cmd_list"
```

---

### Task 3: Add `PortEntry`, restructure `cmd_watch`, add colors and event words

**Files:**
- Modify: `examples/sbprobe.rs`

- [ ] **Step 1: Add the `PortEntry` struct and `Stylize` import**

At the top of `examples/sbprobe.rs`, update the imports:

```rust
use comfy_table::{presets, Attribute, Cell, Color, Table};
use crossterm::style::Stylize;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
```

Add the `PortEntry` struct immediately before `cmd_list`:

```rust
struct PortEntry {
    label: &'static str,
    product: String,
    manufacturer: String,
    serial: String,
}
```

- [ ] **Step 2: Rewrite `cmd_watch` in full**

Replace the entire `cmd_watch` function with:

```rust
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
```

Note: `"connected   "` has 3 trailing spaces so its visible width matches `"disconnected"` (12 chars), keeping subsequent columns aligned.

- [ ] **Step 3: Verify**

```bash
cargo check --example sbprobe
```

Expected: compiles clean.

- [ ] **Step 4: Smoke test**

```bash
cargo run --example sbprobe list
```

Expected: table output unchanged (bold headers, blue product/manufacturer, yellow port).

```bash
cargo run --example sbprobe watch
```

Expected: initial scan prints "connected   " in green for any currently-attached USB serial devices. Plug and unplug a device to confirm "connected   " (green) and "disconnected" (red) lines appear with product, manufacturer, serial, and port columns.

- [ ] **Step 5: Commit**

```bash
git add examples/sbprobe.rs
git commit -m "feat(sbprobe): add USB metadata and colors to cmd_watch"
```
