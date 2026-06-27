# spaceball-rs

> _Rust library for Spaceball and SpaceOrb 360 six-degrees-of-freedom input devices._

`spaceball-rs` gives Rust applications a unified, device-agnostic interface to the Spaceball 1003/2003/3003 and SpaceOrb 360 vintage 3D controllers. Connect either device over USB-serial, call `first()` or `probe(path)`, then consume a stream of normalized `DeviceEvent` values — the same code works with both devices. Motion is pre-scaled to `[-1.0, 1.0]` per second so integrating position is a one-liner: `pos += motion.translation * dt`.

<!-- 🖊 TODO: Set project status — delete the others:
> **Status:** Active development — APIs may change between minor versions.
> **Status:** Stable — breaking changes only on major versions.
> **Status:** Experimental / proof-of-concept — use at your own risk.
-->

---

## Table of Contents

- [Features](#features)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [API](#api)
- [Examples](#examples)
- [Known Limitations](#known-limitations)
- [Contributing](#contributing)
- [License](#license)

---

## Features

- **Unified `SixDofDevice` trait** — identical event loop for Spaceball and SpaceOrb; swap hardware without changing application code
- **Auto-detection** — `probe(path)` identifies the device type; `first()` and `find()` scan all serial ports automatically
- **Normalized motion** — both devices output `NormalizedMotion { translation: [f32;3], rotation: [f32;3] }` in `[-1.0, 1.0]` per second, regardless of device-specific encoding differences
- **Raw packet access** — `packets_with_bytes()` on both `Spaceball` and `SpaceOrb` exposes decoded packets alongside original wire bytes for diagnostics
- **`InputMode` scaling** — `process(&motion, &mode)` applies `ObjectManipulation` or `CameraControl` sensitivity presets to produce ready-to-integrate `ScaledMotion`
- **`sbprobe` utility** — `list` scans ports and identifies devices; `watch` prints a timestamped line on every connect/disconnect

---

## Prerequisites

- **Rust**: edition 2024 (stable ≥ 1.85 or nightly)
- **Hardware**: Spaceball 1003, 2003, or 3003, or a SpaceOrb 360, connected via a USB-serial adapter

No additional system libraries required; `serialport` handles platform serial I/O.

---

## Installation

This crate is not yet published to crates.io. Add it from source:

```toml
# Cargo.toml
[dependencies]
spaceball-rs = { path = "../spaceball-rs" }
```

Or via git:

```toml
[dependencies]
spaceball-rs = { git = "https://github.com/PeteRichardson/spaceball-rs.git" }
```

### Build from source

```sh
git clone https://github.com/PeteRichardson/spaceball-rs.git
cd spaceball-rs
cargo build
```

---

## Quick Start

<!-- 🖊 TODO: Add a screenshot or GIF of sbprobe list / watch output here.
<p align="center">
  <img src="docs/images/demo.gif" alt="sbprobe watch demo" width="700">
</p>
-->

```rust
use spaceball_rs::{first, DeviceEvent, InputMode, process};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut device = first()?;           // auto-detect Spaceball or SpaceOrb
    let mode = InputMode::object_manipulation_default();
    let mut last = Instant::now();
    let mut pos = [0.0f32; 3];

    for event in device.events() {
        if let Ok(DeviceEvent::Motion(m)) = event {
            let dt = last.elapsed().as_secs_f32();
            last = Instant::now();
            let scaled = process(&m, &mode);
            pos[0] += scaled.translation[0] * dt;
            pos[1] += scaled.translation[1] * dt;
            pos[2] += scaled.translation[2] * dt;
            println!("pos: {:?}", pos);
        }
    }
    Ok(())
}
```

---

## API

### Device discovery

| Function | Description |
|----------|-------------|
| `spaceball_rs::probe(path: &str)` | Open a specific port and auto-detect device type; returns `Box<dyn SixDofDevice>` |
| `spaceball_rs::first()` | Scan all USB-serial ports and return the first recognized device |
| `spaceball_rs::find()` | Scan all USB-serial ports and return every recognized device |
| `Spaceball::probe(path)` | Open `path` and confirm a Spaceball is attached |
| `SpaceOrb::probe(path)` | Open `path` and confirm a SpaceOrb 360 is attached |

### `SixDofDevice` trait

```rust
pub trait SixDofDevice: Send {
    fn device_id(&self) -> &'static str;   // "Spaceball" or "SpaceOrb"
    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_>;
}
```

### Event types

```rust
pub enum DeviceEvent {
    Motion(NormalizedMotion),
    Button(Box<dyn ButtonState + Send>),
}

pub struct NormalizedMotion {
    pub translation: [f32; 3],   // [-1.0, 1.0] per second
    pub rotation:    [f32; 3],   // [-1.0, 1.0] per second
}
```

### `InputMode` and `process`

```rust
let mode = InputMode::object_manipulation_default(); // translation_scale=3.0, rotation_scale=2.0
let mode = InputMode::camera_control_default();      // translation_scale=5.0, rotation_scale=1.5

let scaled: ScaledMotion = process(&normalized_motion, &mode);
// scaled.translation and scaled.rotation are in world-units/s and rad/s
// Accumulate as: pos += scaled.translation * dt
```

### Raw packet access

```rust
// Spaceball — CR-terminated packets with ^-escape encoding
for result in device.packets_with_bytes() {
    let rp = result?;   // RawPacket<SpaceballPacket>
    println!("wire: {:?}  packet: {:?}", rp.raw, rp.packet);
}

// SpaceOrb — packets bounded by XOR checksum byte
for result in device.packets_with_bytes() {
    let rp = result?;   // RawPacket<SpaceOrbPacket>
    println!("wire: {:?}  packet: {:?}", rp.raw, rp.packet);
}
```

### Threading pattern

Move the device into a background thread and share state via `Arc<Mutex<_>>`:

```rust
let device = spaceball_rs::first()?;
let state = Arc::new(Mutex::new(MyState::default()));
let state_bg = Arc::clone(&state);

std::thread::spawn(move || {
    for event in device.events() { /* update state_bg */ }
});
```

---

## Examples

### `sbprobe` — port scanner and device monitor

```sh
# List all USB-serial ports and identify attached devices
cargo run --example sbprobe list

# Watch for device connect/disconnect events (runs until Ctrl-C)
cargo run --example sbprobe watch
```

`list` output columns: Device · Product · Manufacturer · Serial · Port

`watch` output format: `[elapsed_s]  connected/disconnected  device  product  manufacturer  serial  port`

### `packetdump` — decoded packet stream

```sh
# Default: Spaceball on the first available port
cargo run --example packetdump

# SpaceOrb on a specific port
cargo run --example packetdump spaceorb /dev/cu.usbserial-0001
```

### `hexdump` — raw wire bytes

```sh
cargo run --example hexdump
cargo run --example hexdump spaceorb /dev/cu.usbserial-0001
```

### `cube` — 3D cube rendered with `three-d`

```sh
cargo run --example cube
cargo run --example cube /dev/cu.usbserial-0001
```

Rotate and translate a 3D cube in real time. Press button 1 to reset the view.

### `asteroids3d` — Asteroids game with Bevy

```sh
cargo run --example asteroids3d
cargo run --example asteroids3d /dev/cu.usbserial-0001
```

---

## Known Limitations

- **macOS only tested in practice** — `candidate_ports()` filters `/dev/tty.*` (macOS dial-in ports that block on DCD) and keeps `/dev/cu.*` and Linux `ttyUSB*`/`ttyACM*`. Untested on Windows; the `serialport` crate supports Windows but no CI validates it.
- **USB-serial adapters only** — the library skips Bluetooth, PCI, and unknown port types when auto-scanning. Devices must be connected via a USB-to-serial adapter.
- **Some adapters ignore RTS/DTR** — `open()` asserts RTS (and DTR for SpaceOrb, which uses these lines as power rails). Adapters that return `EINVAL` for modem control are handled gracefully, but behavior depends on the adapter.
- **No crates.io release** — install from git or a local path; see [Installation](#installation).

<!-- 🖊 TODO: Review and expand — check open issues for additional known limitations. -->

---

## Contributing

Contributions welcome.

```sh
git clone https://github.com/PeteRichardson/spaceball-rs.git
cd spaceball-rs
cargo test
cargo check --examples
```

Please open an issue before starting significant work.

---

## License

<!-- 🖊 TODO: Add a LICENSE file and update this section with the SPDX identifier. -->
License not yet specified — see repository for details.
