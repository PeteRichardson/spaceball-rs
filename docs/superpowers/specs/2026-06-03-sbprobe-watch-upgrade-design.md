# sbprobe cmd_watch upgrade: USB metadata + colors

**Date:** 2026-06-03

## Goal

Bring `cmd_watch` up to parity with `cmd_list`: show product, manufacturer, and serial number on each event line, with the same crossterm colors (product/manufacturer in blue, port in yellow). Detach lines show cached metadata from when the port was first seen. The `+`/`-` event indicator is replaced with the words "connected" (green) and "disconnected" (red) to make attach and detach events more visually distinct.

## Changes

### 1. `usb_fields()` helper

```rust
fn usb_fields(info: &serialport::SerialPortInfo) -> (String, String, String)
```

Extracts `(product, manufacturer, serial)` from a `SerialPortInfo` as owned `String`s, returning `"?"` for any absent field. Replaces the inline `match &info.port_type` block currently in `cmd_list`, and used by `cmd_watch` as well.

### 2. `PortEntry` struct

```rust
struct PortEntry {
    label: &'static str,
    product: String,
    manufacturer: String,
    serial: String,
}
```

`known` in `cmd_watch` changes from `HashMap<String, &'static str>` to `HashMap<String, PortEntry>`, so metadata is available when a port detaches.

### 3. Loop restructure in `cmd_watch`

The new-port detection loop currently converts `candidate_ports()` to a `HashSet<String>` immediately, discarding the `SerialPortInfo`. Since `usb_fields()` needs the full `SerialPortInfo`, the loop is restructured to collect the full `Vec<SerialPortInfo>` first and derive the `HashSet` from it separately.

### 4. Colors via crossterm

`crossterm = "0.29"` added to `[dev-dependencies]` in `Cargo.toml` (matches the version already in the lockfile as a transitive dep of comfy-table). `use crossterm::style::Stylize` provides `.blue()`, `.yellow()`, `.green()`, `.red()` on `&str`. Applied inline at each `println!` in `cmd_watch`.

**ANSI padding rule:** ANSI escape codes inflate byte count, so `{:<N}` format specifiers don't measure visible width correctly when applied to already-colored strings. Always pad first, then color: `format!("{:<20}", product).green()` not `format!("{:<20}", product.green())`. This applies to all colored columns.

The event word is a known-length field: `"disconnected"` is 12 chars; `"connected"` is padded to 12 before coloring (`format!("{:<12}", "connected").green()`) to keep subsequent columns aligned.

### 5. `cmd_list` simplification

`cmd_list` replaces its inline `match &info.port_type` block with a call to `usb_fields()`. No behavioral change.

## Out of scope

- `print_event_line` helper: only two call sites (attach and detach), not worth the indirection.
- Keeping `+`/`-` alongside the words: words alone are sufficient.
- Column width auto-sizing in watch output: fixed-width format strings are appropriate for streaming output.
