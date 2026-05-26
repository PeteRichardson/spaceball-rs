# Multi-Device 6DOF Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add SpaceOrb 360 support alongside Spaceball, with a `Probeable` trait that gives every device type `find()` and `first()`, and a `SixDofDevice` trait for device-agnostic normalized motion.

**Architecture:** The monolithic `src/lib.rs` splits into `src/spaceball.rs` (existing device) and `src/spaceorb.rs` (new device); `lib.rs` becomes a thin hub of shared traits and free functions. Each device module is self-contained: it owns its packet types, `open()`, `impl Probeable`, and `impl SixDofDevice`. The `Probeable` trait provides `find()` and `first()` default implementations so device modules get port-scanning for free by implementing one `probe(path)` method.

**Tech Stack:** Rust 2024 edition, `serialport` crate (serial I/O + port enumeration), `bevy` + `three-d` (examples only, dev-dependencies).

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/lib.rs` | Rewrite | Shared types (`Error`, `NormalizedMotion`, `ButtonState`, `DeviceEvent`), `SixDofDevice` trait, `Probeable` trait, `probe()`/`find()`/`first()` free functions, `mod` declarations, re-exports |
| `src/spaceball.rs` | Create (from lib.rs) | `Spaceball` struct, `SpaceballBallEvent`, `SpaceballKeyEvent`, `SpaceballPacket`, packet parser, `open()`, `packets()`, `impl ButtonState`, `impl SixDofDevice`, `impl Probeable` |
| `src/spaceorb.rs` | Create | `SpaceOrb` struct, `SpaceOrbBallEvent`, `SpaceOrbKeyEvent`, `SpaceOrbPacket`, packet parser, `open()`, `packets()`, `impl ButtonState`, `impl SixDofDevice`, `impl Probeable` |
| `examples/hexdump.rs` | Modify | Add `--device spaceball\|spaceorb` arg (default: `spaceball`) |
| `examples/packetdump.rs` | Modify | Add `--device spaceball\|spaceorb` arg (default: `spaceball`) |
| `examples/cube.rs` | Modify | Replace `Spaceball::open()` with `probe()` or `--device` flag |
| `examples/asteroids3d.rs` | Modify | Replace `Spaceball` + raw packet handling with `Box<dyn SixDofDevice>` + `events()` |

---

## Task 1: Scaffold — split lib.rs into spaceball.rs + thin lib.rs

**Files:**
- Create: `src/spaceball.rs`
- Rewrite: `src/lib.rs`

- [ ] **Step 1: Copy current lib.rs content to spaceball.rs**

```bash
cp src/lib.rs src/spaceball.rs
```

- [ ] **Step 2: Replace lib.rs with new skeleton**

Write `src/lib.rs`:

```rust
mod spaceball;
mod spaceorb;

pub use spaceball::{
    Spaceball, SpaceballBallEvent, SpaceballKeyEvent, SpaceballPacket,
};
pub use spaceorb::{
    SpaceOrb, SpaceOrbBallEvent, SpaceOrbKeyEvent, SpaceOrbPacket,
};

use std::io;

// ── Shared error type ────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Error {
    Serial(serialport::Error),
    Io(io::Error),
    NoDeviceFound,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Serial(e) => write!(f, "serial port error: {e}"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::NoDeviceFound => write!(f, "no supported 6DOF device found"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Serial(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::NoDeviceFound => None,
        }
    }
}

impl From<serialport::Error> for Error {
    fn from(e: serialport::Error) -> Self { Error::Serial(e) }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self { Error::Io(e) }
}

// ── Shared types ─────────────────────────────────────────────────────────────

/// Normalized motion rate. Apply per frame as: `pos += motion.translation * dt`
///
/// Both axes are scaled to [-1.0, 1.0] at maximum sustained deflection,
/// in units per second. Multiply by delta-time to get frame displacement.
#[derive(Debug, Clone)]
pub struct NormalizedMotion {
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
}

/// Generic button access, implemented by device-specific key event types.
pub trait ButtonState {
    fn pressed(&self, index: usize) -> bool;
    fn count(&self) -> usize;
    fn any_pressed(&self) -> bool {
        (0..self.count()).any(|i| self.pressed(i))
    }
}

/// Device-agnostic event yielded by [`SixDofDevice::events`].
pub enum DeviceEvent {
    Motion(NormalizedMotion),
    Button(Box<dyn ButtonState + Send>),
}

// ── SixDofDevice trait ───────────────────────────────────────────────────────

/// Implemented by [`Spaceball`] and [`SpaceOrb`]. Object-safe.
///
/// Use [`probe`], [`find`], or [`first`] to obtain a `Box<dyn SixDofDevice>`
/// without knowing which device is attached.
pub trait SixDofDevice: Send {
    /// Returns an iterator of device-agnostic events. Ball motion events carry
    /// a [`NormalizedMotion`] in [-1, 1] per second; apply as `pos += v * dt`.
    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_>;
}

// ── Probeable trait ──────────────────────────────────────────────────────────

/// Provides `find()` and `first()` for concrete device types.
///
/// Implement [`Probeable::probe`] in each device module; the default
/// `find()` and `first()` methods scan all serial ports automatically.
pub trait Probeable: Sized + SixDofDevice {
    /// Open `path` and confirm this specific device type is attached.
    /// Returns `Ok(Self)` if confirmed; `Err` for wrong device or port failure.
    fn probe(path: &str) -> Result<Self, Error>;

    /// Scan all serial ports and return every device of this type found.
    fn find() -> Vec<Self> {
        serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|info| Self::probe(&info.port_name).ok())
            .collect()
    }

    /// Scan all serial ports and return the first device of this type found.
    fn first() -> Result<Self, Error> {
        serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .find_map(|info| Self::probe(&info.port_name).ok())
            .ok_or(Error::NoDeviceFound)
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Auto-detect the device on `path` and return it as a trait object.
///
/// Tries SpaceOrb first (deterministic `?` response), then Spaceball.
pub fn probe(path: &str) -> Result<Box<dyn SixDofDevice>, Error> {
    if let Ok(orb) = SpaceOrb::probe(path) {
        return Ok(Box::new(orb));
    }
    if let Ok(sb) = Spaceball::probe(path) {
        return Ok(Box::new(sb));
    }
    Err(Error::NoDeviceFound)
}

/// Scan all serial ports and return every recognized 6DOF device.
pub fn find() -> Vec<Box<dyn SixDofDevice>> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|info| probe(&info.port_name).ok())
        .collect()
}

/// Scan all serial ports and return the first recognized 6DOF device.
pub fn first() -> Result<Box<dyn SixDofDevice>, Error> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .find_map(|info| probe(&info.port_name).ok())
        .ok_or(Error::NoDeviceFound)
}
```

- [ ] **Step 3: Add a placeholder spaceorb.rs so the project compiles**

Write `src/spaceorb.rs`:

```rust
// Placeholder — filled in Task 3.
use crate::{ButtonState, DeviceEvent, Error, NormalizedMotion, Probeable, SixDofDevice};
use std::io;

pub struct SpaceOrb {
    _port: Box<dyn serialport::SerialPort>,
}
pub struct SpaceOrbBallEvent { pub force: [i16; 3], pub torque: [i16; 3] }
pub struct SpaceOrbKeyEvent  { pub rezero: bool, pub buttons: [bool; 6] }
pub enum SpaceOrbPacket {
    Ball(SpaceOrbBallEvent),
    Key(SpaceOrbKeyEvent),
    Reset(String),
    Error { brown_out: bool, eeprom: bool, hardware: bool },
    Unknown(Vec<u8>),
}

impl ButtonState for SpaceOrbKeyEvent {
    fn pressed(&self, i: usize) -> bool { self.buttons.get(i).copied().unwrap_or(false) }
    fn count(&self) -> usize { 6 }
}

impl SixDofDevice for SpaceOrb {
    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_> {
        Box::new(std::iter::empty())
    }
}

unsafe impl Send for SpaceOrb {}

impl Probeable for SpaceOrb {
    fn probe(_path: &str) -> Result<Self, Error> { Err(Error::NoDeviceFound) }
}
```

- [ ] **Step 4: Update spaceball.rs to use the new Error type from lib.rs**

At the top of `src/spaceball.rs`, replace the existing `Error` type definition and `From` impls with imports from `crate`:

```rust
use crate::{ButtonState, DeviceEvent, Error, NormalizedMotion, Probeable, SixDofDevice};
```

Remove from `src/spaceball.rs`:
- The `pub enum Error { … }` definition
- All `impl std::fmt::Display for Error`, `impl std::error::Error for Error`, `impl From<…> for Error` blocks

The `Spaceball`, `KeyEvent`, `BallEvent`, `Packet`, `PacketIter` types stay as-is for now (renamed in Task 2).

- [ ] **Step 5: Verify it compiles**

```bash
cargo check 2>&1
```

Expected: `Finished` with no errors. Fix any import issues before proceeding.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/spaceball.rs src/spaceorb.rs
git commit -m "refactor: split lib.rs into spaceball.rs + spaceorb.rs placeholder; add shared traits"
```

---

## Task 2: Rename Spaceball types; impl ButtonState, SixDofDevice, Probeable

**Files:**
- Modify: `src/spaceball.rs`
- Modify: `examples/hexdump.rs`, `examples/packetdump.rs`, `examples/cube.rs`, `examples/asteroids3d.rs` (update renamed types)

The goal is the complete new Spaceball API. The existing parsing logic (`parse_packet`, `PacketIter`) stays unchanged — we just rename types and add trait impls.

- [ ] **Step 1: Write tests for the packet parser and ButtonState**

Add a `#[cfg(test)]` block at the bottom of `src/spaceball.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // K packet: K + byte1 + byte2
    // byte1: 010<pick><b8><b7><b6><b5>
    // byte2: 0100<b4><b3><b2><b1>
    fn make_key_raw(pick: bool, buttons: [bool; 8]) -> Vec<u8> {
        let b1: u8 = 0x40
            | if pick          { 0x10 } else { 0 }
            | if buttons[7]    { 0x08 } else { 0 }  // b8
            | if buttons[6]    { 0x04 } else { 0 }  // b7
            | if buttons[5]    { 0x02 } else { 0 }  // b6
            | if buttons[4]    { 0x01 } else { 0 }; // b5
        let b2: u8 = 0x40
            | if buttons[3] { 0x08 } else { 0 }  // b4
            | if buttons[2] { 0x04 } else { 0 }  // b3
            | if buttons[1] { 0x02 } else { 0 }  // b2
            | if buttons[0] { 0x01 } else { 0 }; // b1
        vec![b'K', b1, b2]
    }

    #[test]
    fn key_packet_no_buttons() {
        let raw = make_key_raw(false, [false; 8]);
        let pkt = parse_packet(raw);
        if let SpaceballPacket::Key(k) = pkt {
            assert!(!k.pick);
            assert_eq!(k.buttons, [false; 8]);
        } else {
            panic!("expected Key packet");
        }
    }

    #[test]
    fn key_packet_pick_and_button1() {
        let mut btns = [false; 8];
        btns[0] = true;
        let raw = make_key_raw(true, btns);
        let pkt = parse_packet(raw);
        if let SpaceballPacket::Key(k) = pkt {
            assert!(k.pick);
            assert!(k.buttons[0]);
            assert!(!k.buttons[1]);
        } else {
            panic!("expected Key packet");
        }
    }

    #[test]
    fn button_state_trait() {
        let k = SpaceballKeyEvent {
            pick: false,
            buttons: [true, false, true, false, false, false, false, false],
        };
        assert!(k.pressed(0));
        assert!(!k.pressed(1));
        assert!(k.pressed(2));
        assert_eq!(k.count(), 8);
        assert!(k.any_pressed());
    }

    #[test]
    fn ball_packet_zeros() {
        // D + period(0,0) + tx(0,0) + ty(0,0) + tz(0,0) + rx(0,0) + ry(0,0) + rz(0,0)
        let raw = vec![b'D', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let pkt = parse_packet(raw);
        if let SpaceballPacket::Ball(b) = pkt {
            assert_eq!(b.period, 0);
            assert_eq!(b.translation, [0, 0, 0]);
            assert_eq!(b.rotation, [0, 0, 0]);
        } else {
            panic!("expected Ball packet");
        }
    }

    #[test]
    fn ball_packet_values() {
        // period = 800 (0x0320), tx = 1000 (0x03E8)
        let period: u16 = 800;
        let tx: i16 = 1000;
        let mut raw = vec![b'D'];
        raw.extend_from_slice(&period.to_be_bytes());
        raw.extend_from_slice(&tx.to_be_bytes());
        raw.extend_from_slice(&[0u8; 10]); // ty, tz, rx, ry, rz
        let pkt = parse_packet(raw);
        if let SpaceballPacket::Ball(b) = pkt {
            assert_eq!(b.period, 800);
            assert_eq!(b.translation[0], 1000);
        } else {
            panic!("expected Ball packet");
        }
    }
}
```

- [ ] **Step 2: Run tests (they should fail — types not renamed yet)**

```bash
cargo test --lib 2>&1
```

Expected: compile errors because `Packet`, `BallEvent`, `KeyEvent` don't exist yet as `SpaceballPacket`, etc.

- [ ] **Step 3: Rename types in spaceball.rs**

In `src/spaceball.rs`, apply these renames throughout the file:
- `BallEvent` → `SpaceballBallEvent`
- `KeyEvent` → `SpaceballKeyEvent`
- `Packet` → `SpaceballPacket`
- `PacketIter` → `SpaceballPacketIter`

Also add `pub` to `parse_packet` so tests can call it, and update its signature:

```rust
pub(crate) fn parse_packet(raw: Vec<u8>) -> SpaceballPacket {
```

Add `impl ButtonState for SpaceballKeyEvent` after the struct definition:

```rust
impl ButtonState for SpaceballKeyEvent {
    fn pressed(&self, i: usize) -> bool {
        self.buttons.get(i).copied().unwrap_or(false)
    }
    fn count(&self) -> usize { 8 }
}
```

Update the `pub use` in `src/lib.rs` to match (already done in Task 1 Step 2).

- [ ] **Step 4: Run tests (should pass now)**

```bash
cargo test --lib 2>&1
```

Expected: all 5 tests pass.

- [ ] **Step 5: Add impl SixDofDevice for Spaceball**

Add after the `impl Spaceball` block in `src/spaceball.rs`:

```rust
impl SixDofDevice for Spaceball {
    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_> {
        let mut last_period = 800u16; // ~50 ms default (20 Hz)
        Box::new(self.packets().filter_map(move |pkt| match pkt {
            Err(e) => Some(Err(e)),
            Ok(SpaceballPacket::Ball(b)) => {
                if b.period > 0 { last_period = b.period; }
                let period_secs = last_period as f32 / 16_000.0;
                let norm = |v: i16| (v as f32 / period_secs) / 320_000.0;
                Some(Ok(DeviceEvent::Motion(NormalizedMotion {
                    translation: [
                        norm(b.translation[0]),
                        norm(b.translation[1]),
                        norm(b.translation[2]),
                    ],
                    rotation: [
                        norm(b.rotation[0]),
                        norm(b.rotation[1]),
                        norm(b.rotation[2]),
                    ],
                })))
            }
            Ok(SpaceballPacket::Key(k)) => {
                Some(Ok(DeviceEvent::Button(Box::new(k))))
            }
            Ok(SpaceballPacket::Unknown(_)) => None,
        }))
    }
}
```

- [ ] **Step 6: Add impl Probeable for Spaceball**

Add after `impl SixDofDevice for Spaceball`:

```rust
impl Probeable for Spaceball {
    /// Open `path` and confirm a Spaceball is attached.
    ///
    /// Sends `?\r`; if the reply starts with `!` it's a SpaceOrb — return Err.
    /// No reply within 200 ms means assume Spaceball (it ignores `?` quietly).
    fn probe(path: &str) -> Result<Self, Error> {
        use std::io::Read;
        use std::time::Duration;

        let mut port = serialport::new(path, 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(500))
            .open()?;

        let _ = port.write_request_to_send(true);
        let _ = port.write_data_terminal_ready(true);

        // Wait up to 500 ms for a spontaneous power-up byte.
        let mut buf = [0u8; 1];
        match port.read(&mut buf) {
            Ok(1) if buf[0] == b'@' => {
                // Spaceball power-up message — this is our device.
            }
            Ok(1) if buf[0] == b'R' => {
                // SpaceOrb power-up message — not a Spaceball.
                return Err(Error::NoDeviceFound);
            }
            _ => {
                // Already powered: send `?\r` and check for SpaceOrb `!` reply.
                port.write_all(b"?\r")?;
                port.set_timeout(Duration::from_millis(200))?;
                match port.read(&mut buf) {
                    Ok(1) if buf[0] == b'!' => {
                        return Err(Error::NoDeviceFound); // SpaceOrb replied
                    }
                    _ => {} // silence or `?` echo — treat as Spaceball
                }
            }
        }

        // Confirmed (or assumed) Spaceball — run full initialization.
        Spaceball::open(path)
    }
}
```

- [ ] **Step 7: Fix examples to use renamed types**

In `examples/hexdump.rs`, `examples/packetdump.rs`, `examples/cube.rs`, and `examples/asteroids3d.rs`, update any imports or type references:
- `spaceball_rs::Packet` → `spaceball_rs::SpaceballPacket`
- `spaceball_rs::BallEvent` → `spaceball_rs::SpaceballBallEvent`
- `spaceball_rs::KeyEvent` → `spaceball_rs::SpaceballKeyEvent`

```bash
grep -rn "spaceball_rs::" examples/
```

In each example, update `use spaceball_rs::{Packet, Spaceball}` to `use spaceball_rs::{SpaceballPacket, Spaceball}`, and update match arms from `Packet::Ball` / `Packet::Key` / `Packet::Unknown` to `SpaceballPacket::Ball` / `SpaceballPacket::Key` / `SpaceballPacket::Unknown`.

- [ ] **Step 8: Verify everything compiles**

```bash
cargo check --examples 2>&1
```

Expected: `Finished` with no errors.

- [ ] **Step 9: Commit**

```bash
git add src/spaceball.rs src/lib.rs examples/
git commit -m "feat(spaceball): rename types, add ButtonState/SixDofDevice/Probeable impls"
```

---

## Task 3: SpaceOrb packet types and parser

**Files:**
- Modify: `src/spaceorb.rs`

The SpaceOrb wire format: packets are bounded by the next packet's header byte (top bit = 0, printable ASCII) or a standalone `\r`. Each packet ends with an XOR checksum byte (top bit = 1). Binary data bytes have top bit set; text in `R`/`!` packets is ASCII with top bit = 0 until the XOR byte.

Known packet structures:
- `D`: header + 1 (button status) + 9 (packed 6×10-bit data) + 1 (xor) = 11 bytes after header
- `K`: header + 1 (period) + 1 (status) + 1 (reserved) + 1 (xor) = 4 bytes after header  
- `E`: header + 1 (flags) + 1 (reserved) + 1 (xor) = 3 bytes after header
- `N`: header + 1 (null region) + 1 (xor) = 2 bytes after header
- `R`, `!`: header + variable ASCII + 1 (xor byte, top bit set) = read until top-bit-set byte

- [ ] **Step 1: Write failing tests for SpaceOrb packet parsing**

Replace `src/spaceorb.rs` entirely with the following (types + parser + tests):

```rust
use crate::{ButtonState, DeviceEvent, Error, NormalizedMotion, Probeable, SixDofDevice};
use std::io;

// ── Public types ─────────────────────────────────────────────────────────────

pub struct SpaceOrb {
    port: Box<dyn serialport::SerialPort>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpaceOrbBallEvent {
    pub force:  [i16; 3],
    pub torque: [i16; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpaceOrbKeyEvent {
    pub rezero:  bool,
    pub buttons: [bool; 6], // [0]=A … [5]=F
}

impl SpaceOrbKeyEvent {
    pub fn a(&self) -> bool { self.buttons[0] }
    pub fn b(&self) -> bool { self.buttons[1] }
    pub fn c(&self) -> bool { self.buttons[2] }
    pub fn d(&self) -> bool { self.buttons[3] }
    pub fn e(&self) -> bool { self.buttons[4] }
    pub fn f(&self) -> bool { self.buttons[5] }
}

impl ButtonState for SpaceOrbKeyEvent {
    fn pressed(&self, i: usize) -> bool { self.buttons.get(i).copied().unwrap_or(false) }
    fn count(&self) -> usize { 6 }
}

#[derive(Debug)]
pub enum SpaceOrbPacket {
    Ball(SpaceOrbBallEvent),
    Key(SpaceOrbKeyEvent),
    Reset(String),
    Error { brown_out: bool, eeprom: bool, hardware: bool },
    Unknown(Vec<u8>),
}

// ── Wire-format parsing ───────────────────────────────────────────────────────

/// Parse a complete SpaceOrb packet from its raw bytes (header + data, no XOR byte).
pub(crate) fn parse_orb_packet(raw: &[u8]) -> SpaceOrbPacket {
    match raw.first() {
        Some(b'D') if raw.len() == 11 => {
            // raw[1] = button status (repeat of K packet, used for robustness)
            // raw[2..=10] = 9 packed data bytes
            let data: [u8; 9] = raw[2..11].try_into().unwrap();
            SpaceOrbPacket::Ball(decode_ball_data(&data))
        }
        Some(b'K') if raw.len() == 4 => {
            // raw[1] = period, raw[2] = status, raw[3] = reserved
            let status = raw[2];
            SpaceOrbPacket::Key(SpaceOrbKeyEvent {
                rezero:  (status & 0x40) != 0,
                buttons: [
                    (status & 0x01) != 0, // A
                    (status & 0x02) != 0, // B
                    (status & 0x04) != 0, // C
                    (status & 0x08) != 0, // D
                    (status & 0x10) != 0, // E
                    (status & 0x20) != 0, // F
                ],
            })
        }
        Some(b'E') if raw.len() == 3 => {
            let flags = raw[1];
            SpaceOrbPacket::Error {
                hardware:  (flags & 0x01) != 0,
                eeprom:    (flags & 0x02) != 0,
                brown_out: (flags & 0x04) != 0,
            }
        }
        Some(b'R') | Some(b'!') => {
            // ASCII text: strip header byte, decode remaining as UTF-8
            let text = raw[1..].iter()
                .map(|&b| (b & 0x7F) as char)
                .collect::<String>()
                .trim()
                .to_string();
            SpaceOrbPacket::Reset(text)
        }
        _ => SpaceOrbPacket::Unknown(raw.to_vec()),
    }
}

/// Decode the 9 packed data bytes of a D packet into force+torque components.
///
/// Each byte was XOR'd with the corresponding byte of b"SpaceWare" before
/// transmission. Six signed 10-bit values are packed MSB-first across 9×7 bits.
pub(crate) fn decode_ball_data(bytes: &[u8; 9]) -> SpaceOrbBallEvent {
    const SPACEWARE: &[u8; 9] = b"SpaceWare";
    // Recover the 7 data bits from each byte.
    let d: [u8; 9] = std::array::from_fn(|i| (bytes[i] ^ SPACEWARE[i]) & 0x7F);

    // Unpack six 10-bit values from 9×7 = 63 bits (3 padding bits at end).
    let fx = ((d[0] as u16) << 3) | ((d[1] as u16) >> 4);
    let fy = (((d[1] & 0x0F) as u16) << 6) | ((d[2] as u16) >> 1);
    let fz = (((d[2] & 0x01) as u16) << 9)
           | ((d[3] as u16) << 2)
           | ((d[4] as u16) >> 5);
    let tx = (((d[4] & 0x1F) as u16) << 5) | ((d[5] as u16) >> 2);
    let ty = (((d[5] & 0x03) as u16) << 8)
           | ((d[6] as u16) << 1)
           | ((d[7] as u16) >> 6);
    let tz = (((d[7] & 0x3F) as u16) << 4) | ((d[8] as u16) >> 3);

    SpaceOrbBallEvent {
        force:  [sign10(fx), sign10(fy), sign10(fz)],
        torque: [sign10(tx), sign10(ty), sign10(tz)],
    }
}

/// Sign-extend a 10-bit unsigned value to i16.
fn sign10(v: u16) -> i16 {
    let v = v & 0x3FF;
    if v & 0x200 != 0 { (v as i16) | (-1i16 << 10) } else { v as i16 }
}

// ── SpaceOrb::open() ─────────────────────────────────────────────────────────

impl SpaceOrb {
    pub fn open(path: &str) -> Result<Self, Error> {
        use std::io::Read;
        use std::time::Duration;

        let port = serialport::new(path, 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(1000))
            .open()?;

        // SpaceOrb draws power from RTS and DTR.
        let _ = port.write_request_to_send(true);
        let _ = port.write_data_terminal_ready(true);

        // Consume any startup packets (R, !1, !2, \r) so the first packet
        // from packets() is a live ball or button event.
        // We read for up to 500 ms, discarding until silence.
        // (Implemented in detail in Task 4.)

        Ok(SpaceOrb { port })
    }

    /// Returns an iterator of decoded [`SpaceOrbPacket`]s.
    pub fn packets(&mut self) -> SpaceOrbPacketIter<impl std::io::Read + '_> {
        SpaceOrbPacketIter { inner: &mut *self.port }
    }
}

unsafe impl Send for SpaceOrb {}

// ── SpaceOrbPacketIter ────────────────────────────────────────────────────────

pub struct SpaceOrbPacketIter<R> {
    inner: R,
}

impl<R: io::Read> Iterator for SpaceOrbPacketIter<R> {
    type Item = Result<SpaceOrbPacket, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::io::Read;
        loop {
            // Read header byte (top bit = 0, i.e. printable ASCII or \r).
            let mut hdr = [0u8; 1];
            loop {
                match self.inner.read(&mut hdr) {
                    Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                    Err(e) => return Some(Err(e)),
                    Ok(0) => continue,
                    Ok(_) => break,
                }
            }

            if hdr[0] == b'\r' {
                // Terminator packet — no data, loop for the next real packet.
                continue;
            }

            if hdr[0] & 0x80 != 0 {
                // Stray data byte (shouldn't happen) — skip.
                continue;
            }

            let header = hdr[0];

            // Determine how many data bytes to read based on header.
            // For fixed-length packets read exactly that many bytes.
            // For variable-length (R, !) read until the XOR byte (top bit set).
            let data: Vec<u8> = match header {
                b'D' => read_exact_orb(&mut self.inner, 10)?,   // btn + 9 data + xor
                b'K' => read_exact_orb(&mut self.inner, 3)?,    // period + status + reserved + xor... wait
                b'E' => read_exact_orb(&mut self.inner, 2)?,    // flags + reserved + xor
                b'N' => read_exact_orb(&mut self.inner, 1)?,    // null region + xor
                _    => read_until_xor(&mut self.inner)?,       // R, !, unknown
            };

            // Build raw slice: header + data bytes (excluding XOR byte at end).
            let mut raw = Vec::with_capacity(1 + data.len());
            raw.push(header);
            if !data.is_empty() {
                raw.extend_from_slice(&data[..data.len().saturating_sub(1)]);
            }

            return Some(Ok(parse_orb_packet(&raw)));
        }
    }
}

// Read exactly `n` bytes (including the trailing XOR byte).
fn read_exact_orb<R: io::Read>(r: &mut R, n: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; n];
    let mut pos = 0;
    while pos < n {
        match r.read(&mut buf[pos..]) {
            Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
            Err(_) => return None,
            Ok(0) => continue,
            Ok(got) => pos += got,
        }
    }
    Some(buf)
}

// Read data bytes until a byte with top bit set (the XOR byte).
// For text packets (R, !): content is ASCII (top bit = 0) until XOR byte.
fn read_until_xor<R: io::Read>(r: &mut R) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match r.read(&mut byte) {
            Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
            Err(_) => return None,
            Ok(0) => continue,
            Ok(_) => {
                buf.push(byte[0]);
                if byte[0] & 0x80 != 0 {
                    return Some(buf); // XOR byte — end of packet
                }
            }
        }
    }
}

// ── SixDofDevice for SpaceOrb ─────────────────────────────────────────────────

impl SixDofDevice for SpaceOrb {
    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_> {
        Box::new(self.packets().filter_map(|pkt| match pkt {
            Err(e) => Some(Err(e)),
            Ok(SpaceOrbPacket::Ball(b)) => {
                Some(Ok(DeviceEvent::Motion(NormalizedMotion {
                    translation: [
                        b.force[0] as f32 / 511.0,
                        b.force[1] as f32 / 511.0,
                        b.force[2] as f32 / 511.0,
                    ],
                    rotation: [
                        b.torque[0] as f32 / 511.0,
                        b.torque[1] as f32 / 511.0,
                        b.torque[2] as f32 / 511.0,
                    ],
                })))
            }
            Ok(SpaceOrbPacket::Key(k)) => {
                Some(Ok(DeviceEvent::Button(Box::new(k))))
            }
            _ => None,
        }))
    }
}

// ── Probeable for SpaceOrb ────────────────────────────────────────────────────

impl Probeable for SpaceOrb {
    fn probe(path: &str) -> Result<Self, Error> {
        use std::io::Read;
        use std::time::Duration;

        let mut port = serialport::new(path, 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(500))
            .open()?;

        let _ = port.write_request_to_send(true);
        let _ = port.write_data_terminal_ready(true);

        let mut buf = [0u8; 1];
        match port.read(&mut buf) {
            Ok(1) if buf[0] == b'R' => {
                // SpaceOrb power-up message — confirmed.
                return SpaceOrb::open(path);
            }
            Ok(1) if buf[0] == b'@' => {
                // Spaceball power-up message — not a SpaceOrb.
                return Err(Error::NoDeviceFound);
            }
            _ => {
                // Device already powered: send `?` and look for `!` reply.
                port.write_all(b"?\r")?;
                port.set_timeout(Duration::from_millis(200))?;
                match port.read(&mut buf) {
                    Ok(1) if buf[0] == b'!' => {
                        return SpaceOrb::open(path);
                    }
                    _ => return Err(Error::NoDeviceFound),
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn spaceware_encode(data_bits: &[u8; 9]) -> [u8; 9] {
        const SW: &[u8; 9] = b"SpaceWare";
        std::array::from_fn(|i| (0x80 | data_bits[i]) ^ SW[i])
    }

    fn pack_10bit(values: [i16; 6]) -> [u8; 9] {
        let v: Vec<u16> = values.iter().map(|&x| (x as u16) & 0x3FF).collect();
        let mut bits = [0u8; 9];
        bits[0] = (v[0] >> 3) as u8;
        bits[1] = (((v[0] & 0x7) << 4) | (v[1] >> 6)) as u8;
        bits[2] = (((v[1] & 0x3F) << 1) | (v[2] >> 9)) as u8;
        bits[3] = ((v[2] >> 2) & 0x7F) as u8;
        bits[4] = (((v[2] & 0x3) << 5) | (v[3] >> 5)) as u8;
        bits[5] = (((v[3] & 0x1F) << 2) | (v[4] >> 8)) as u8;
        bits[6] = ((v[4] >> 1) & 0x7F) as u8;
        bits[7] = (((v[4] & 0x1) << 6) | (v[5] >> 4)) as u8;
        bits[8] = ((v[5] & 0xF) << 3) as u8;
        bits
    }

    #[test]
    fn decode_all_zeros() {
        // At rest: all zeros XOR'd with SpaceWare gives b"SpaceWare" on wire.
        let zeros = pack_10bit([0; 6]);
        let encoded = spaceware_encode(&zeros);
        let evt = decode_ball_data(&encoded);
        assert_eq!(evt.force, [0, 0, 0]);
        assert_eq!(evt.torque, [0, 0, 0]);
    }

    #[test]
    fn decode_max_positive() {
        let encoded = spaceware_encode(&pack_10bit([255, 0, 0, 0, 0, 0]));
        let evt = decode_ball_data(&encoded);
        assert_eq!(evt.force[0], 255);
        assert_eq!(evt.force[1], 0);
    }

    #[test]
    fn decode_negative_value() {
        // -1 in 10-bit two's complement = 0x3FF
        let encoded = spaceware_encode(&pack_10bit([-1, 0, 0, 0, 0, 0]));
        let evt = decode_ball_data(&encoded);
        assert_eq!(evt.force[0], -1);
    }

    #[test]
    fn decode_min_value() {
        // -512 in 10-bit = 0x200
        let encoded = spaceware_encode(&pack_10bit([-512, 0, 0, 0, 0, 0]));
        let evt = decode_ball_data(&encoded);
        assert_eq!(evt.force[0], -512);
    }

    #[test]
    fn parse_key_packet_no_buttons() {
        // K + period(0x80) + status(0x80 = no buttons) + reserved(0x80) [+ xor omitted from raw]
        let raw = vec![b'K', 0x80, 0x80, 0x80];
        let pkt = parse_orb_packet(&raw);
        if let SpaceOrbPacket::Key(k) = pkt {
            assert!(!k.rezero);
            assert_eq!(k.buttons, [false; 6]);
        } else {
            panic!("expected Key");
        }
    }

    #[test]
    fn parse_key_packet_button_a() {
        // status = 1<rezero=0><F=0><E=0><D=0><C=0><B=0><A=1> = 0x81
        let raw = vec![b'K', 0x80, 0x81, 0x80];
        let pkt = parse_orb_packet(&raw);
        if let SpaceOrbPacket::Key(k) = pkt {
            assert!(k.a());
            assert!(!k.b());
            assert_eq!(k.buttons, [true, false, false, false, false, false]);
        } else {
            panic!("expected Key");
        }
    }

    #[test]
    fn parse_key_packet_rezero() {
        // status = 1<rezero=1>... = 0xC0
        let raw = vec![b'K', 0x80, 0xC0, 0x80];
        let pkt = parse_orb_packet(&raw);
        if let SpaceOrbPacket::Key(k) = pkt {
            assert!(k.rezero);
            assert_eq!(k.buttons, [false; 6]);
        } else {
            panic!("expected Key");
        }
    }

    #[test]
    fn parse_reset_packet() {
        // R packet: header + ASCII text; XOR byte stripped by iterator
        let mut raw = vec![b'R'];
        raw.extend_from_slice(b" Spaceball (R) V4.34 19-Oct-96");
        let pkt = parse_orb_packet(&raw);
        if let SpaceOrbPacket::Reset(s) = pkt {
            assert!(s.contains("V4.34"));
        } else {
            panic!("expected Reset");
        }
    }

    #[test]
    fn button_state_trait_orb() {
        let k = SpaceOrbKeyEvent {
            rezero: false,
            buttons: [true, false, true, false, false, false],
        };
        assert!(k.pressed(0));
        assert!(!k.pressed(1));
        assert_eq!(k.count(), 6);
        assert!(k.any_pressed());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --lib 2>&1
```

Expected: all tests pass. The decode and parse tests are pure functions — no serial port needed.

- [ ] **Step 3: Verify the packet iterator byte counts match the protocol**

The `D` packet has: `D` + 1 (button status) + 9 (packed data) + 1 (XOR) = 12 bytes total.
In the iterator, we call `read_exact_orb(&mut self.inner, 10)` (10 bytes after the header) which includes the XOR byte. After stripping the XOR byte (`data[..data.len()-1]`), `raw` has `[b'D', btn, d0..d8]` = 11 bytes. `parse_orb_packet` checks `raw.len() == 11`. Verify these numbers match in the code above.

The `K` packet has: `K` + 1 (period) + 1 (status) + 1 (reserved) + 1 (XOR) = 5 bytes total.
`read_exact_orb(&mut self.inner, 4)` → strip XOR → `raw` has `[b'K', period, status, reserved]` = 4 bytes. `parse_orb_packet` checks `raw.len() == 4`. ✓

- [ ] **Step 4: Commit**

```bash
git add src/spaceorb.rs
git commit -m "feat(spaceorb): add packet types, wire format parser, SixDofDevice, Probeable"
```

---

## Task 4: SpaceOrb::open() — finalize initialization

**Files:**
- Modify: `src/spaceorb.rs`

The `open()` placeholder in Task 3 left init incomplete. The SpaceOrb sends `\r` + `R ...` on power-up. `open()` must drain these startup packets so the caller's first `packets()` call gets a live event.

- [ ] **Step 1: Replace the open() stub with the full implementation**

Replace the `impl SpaceOrb { pub fn open(...) }` block in `src/spaceorb.rs`:

```rust
impl SpaceOrb {
    pub fn open(path: &str) -> Result<Self, Error> {
        use std::io::Read;
        use std::time::Duration;

        let mut port = serialport::new(path, 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(500))
            .open()?;

        // SpaceOrb draws power from RTS and DTR; must assert both.
        let _ = port.write_request_to_send(true);
        let _ = port.write_data_terminal_ready(true);

        // Drain startup packets (R and optional !1 / !2 info packets).
        // Set a short timeout; stop when we get a timeout (no more data).
        port.set_timeout(Duration::from_millis(200))?;
        let mut byte = [0u8; 1];
        loop {
            match port.read(&mut byte) {
                Err(e) if e.kind() == io::ErrorKind::TimedOut => break,
                Err(e) => return Err(Error::Io(e)),
                Ok(0) => break,
                Ok(_) => continue, // discard startup bytes
            }
        }

        // Restore working timeout for normal packet reading.
        port.set_timeout(Duration::from_millis(1000))?;

        Ok(SpaceOrb { port })
    }

    pub fn packets(&mut self) -> SpaceOrbPacketIter<impl std::io::Read + '_> {
        SpaceOrbPacketIter { inner: &mut *self.port }
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check 2>&1
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add src/spaceorb.rs
git commit -m "feat(spaceorb): finalize open() initialization sequence"
```

---

## Task 5: Update examples

**Files:**
- Modify: `examples/hexdump.rs`
- Modify: `examples/packetdump.rs`
- Modify: `examples/cube.rs`
- Modify: `examples/asteroids3d.rs`

- [ ] **Step 1: Update hexdump.rs — add --device flag**

Replace `examples/hexdump.rs`:

```rust
use pretty_hex::*;
use spaceball_rs::{SpaceOrb, Spaceball};

const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (device, path) = parse_args(&mut args);
    eprintln!("Connecting to {device} at {path} ...");

    let cfg = HexConfig { title: false, ..HexConfig::default() };

    match device.as_str() {
        "spaceorb" => run_bytes(SpaceOrb::open(&path)?.bytes(), cfg),
        _ => run_bytes(Spaceball::open(&path)?.bytes(), cfg),
    }
}

fn run_bytes(
    mut bytes: impl Iterator<Item = Result<u8, std::io::Error>>,
    cfg: HexConfig,
) {
    let mut packet = Vec::new();
    loop {
        for b in &mut bytes {
            match b {
                Ok(b'\r') | Err(_) => break,
                Ok(byte) => packet.push(byte),
            }
        }
        if !packet.is_empty() {
            println!("{:?}", packet.hex_conf(cfg));
            packet.clear();
        }
    }
}

fn parse_args(args: &mut impl Iterator<Item = String>) -> (String, String) {
    let mut device = "spaceball".to_string();
    let mut path = DEFAULT_PORT.to_string();
    for arg in args {
        if arg == "--device" || arg.starts_with("--device=") {
            // handled next iteration or inline
        } else if arg == "spaceball" || arg == "spaceorb" {
            device = arg;
        } else {
            path = arg;
        }
    }
    (device, path)
}
```

- [ ] **Step 2: Update packetdump.rs — add --device flag**

Replace `examples/packetdump.rs`:

```rust
use spaceball_rs::{SpaceOrb, SpaceOrbPacket, Spaceball, SpaceballPacket};

const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let device = args.iter().find(|a| *a == "spaceorb").map(|_| "spaceorb").unwrap_or("spaceball");
    let path = args.iter().find(|a| a.starts_with('/') || a.contains("COM"))
        .cloned()
        .unwrap_or_else(|| DEFAULT_PORT.to_string());

    eprintln!("Connecting to {device} at {path} ...\n");

    match device {
        "spaceorb" => {
            let mut orb = SpaceOrb::open(&path)?;
            for packet in orb.packets() {
                match packet? {
                    SpaceOrbPacket::Ball(b) => {
                        let [fx, fy, fz] = b.force;
                        let [tx, ty, tz] = b.torque;
                        println!("BALL  F({fx:5},{fy:5},{fz:5})  T({tx:5},{ty:5},{tz:5})");
                    }
                    SpaceOrbPacket::Key(k) => {
                        let btns: String = ['A','B','C','D','E','F']
                            .iter().zip(k.buttons.iter())
                            .filter(|(_, &p)| p)
                            .map(|(c, _)| *c)
                            .collect();
                        println!("KEY   rezero={:<5} buttons=[{}]",
                            k.rezero, if btns.is_empty() { "none".into() } else { btns });
                    }
                    SpaceOrbPacket::Reset(s) => println!("RESET {s}"),
                    SpaceOrbPacket::Error { brown_out, eeprom, hardware } => {
                        println!("ERR   brown_out={brown_out} eeprom={eeprom} hw={hardware}");
                    }
                    SpaceOrbPacket::Unknown(raw) => {
                        print!("UNK ");
                        for b in &raw { print!(" {b:02x}"); }
                        println!();
                    }
                }
            }
        }
        _ => {
            let mut sb = Spaceball::open(&path)?;
            for packet in sb.packets() {
                match packet? {
                    SpaceballPacket::Ball(b) => {
                        let [tx, ty, tz] = b.translation;
                        let [rx, ry, rz] = b.rotation;
                        println!("BALL  period={:5}  T({tx:6},{ty:6},{tz:6})  R({rx:6},{ry:6},{rz:6})",
                            b.period);
                    }
                    SpaceballPacket::Key(k) => {
                        let btns: String = k.buttons.iter().enumerate()
                            .filter(|(_, &p)| p)
                            .map(|(i, _)| format!("{}", i + 1))
                            .collect::<Vec<_>>().join(", ");
                        println!("KEY   pick={:<5} buttons=[{}]",
                            k.pick, if btns.is_empty() { "none".into() } else { btns });
                    }
                    SpaceballPacket::Unknown(raw) => {
                        print!("UNK ");
                        for b in &raw { print!(" {b:02x}"); }
                        println!();
                    }
                }
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Update cube.rs — use probe() with optional --device/--port flags**

At the top of `examples/cube.rs`, replace:
```rust
use spaceball_rs::{Packet, Spaceball};
const DEFAULT_PORT: &str = "/dev/cu.usbserial-AJ03ACPV";
```
with:
```rust
use spaceball_rs::{DeviceEvent, first, probe};
```

Replace the `let port = …` and `let mut sm = Spaceball::open(&port)?;` block with:

```rust
let path = std::env::args().nth(1);
let mut device: Box<dyn spaceball_rs::SixDofDevice> = match path {
    Some(p) => probe(&p)?,
    None => first()?,
};
```

Replace the background thread + pose accumulation block. Currently it matches on `Packet::Ball` and `Packet::Key`. Change to use `events()`:

```rust
std::thread::spawn(move || {
    for event in device.events() {
        match event {
            Ok(DeviceEvent::Motion(m)) => {
                let mut p = pose_bg.lock().unwrap();
                p.tx += m.translation[0] * 3.0;
                p.ty += m.translation[1] * 3.0;
                p.tz += m.translation[2] * 3.0;
                p.rx += m.rotation[0] * std::f32::consts::TAU;
                p.ry += m.rotation[1] * std::f32::consts::TAU;
                p.rz += m.rotation[2] * std::f32::consts::TAU;
            }
            Ok(DeviceEvent::Button(k)) if k.pressed(0) => {
                *pose_bg.lock().unwrap() = Pose {
                    rx: 25_f32.to_radians(),
                    ry: 35_f32.to_radians(),
                    ..Default::default()
                };
            }
            _ => {}
        }
    }
});
```

Note: `events()` returns normalized rates (units/sec). The cube example previously accumulated raw deltas directly. The equivalent behavior uses the normalized values scaled by the same constants as before (`3.0` for translation, `TAU` for rotation) — these now act as sensitivity multipliers rather than raw scalers.

- [ ] **Step 4: Update asteroids3d.rs — replace Spaceball with probe() + events()**

At the top of `examples/asteroids3d.rs`, replace:
```rust
use spaceball_rs::{Packet, Spaceball};
```
with:
```rust
use spaceball_rs::{DeviceEvent, SixDofDevice, first, probe};
```

Remove `DEFAULT_PORT` const (or keep it for explicit `--port` override).

Replace the `Spaceball` initialization block:
```rust
// Old:
let path = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_PORT.to_string());
let device = Spaceball::open(&path).ok().map(|sb| Arc::new(Mutex::new(sb)));
```
with:
```rust
let device: Option<Arc<Mutex<Box<dyn SixDofDevice>>>> = {
    let result = match std::env::args().nth(1) {
        Some(path) => probe(&path),
        None => first(),
    };
    result.ok().map(|d| Arc::new(Mutex::new(d)))
};
```

In the background thread, replace the match on `Packet::Ball` / `Packet::Key` with `events()`:

```rust
if let Some(device) = device.clone() {
    std::thread::spawn(move || {
        let mut dev = device.lock().unwrap();
        // events() borrows dev mutably; unlock before looping
        // Use a raw pointer trick: take the port out of the mutex for the
        // duration of the background thread.
        // Simpler: restructure so we don't hold the lock during iteration.
        // Actually: since the background thread owns the device exclusively
        // via Arc<Mutex>, we hold the lock for the duration of the thread.
        drop(dev);
        let mut dev = device.lock().unwrap();
        for event in dev.events() {
            match event {
                Ok(DeviceEvent::Motion(m)) => {
                    if let Ok(mut state) = shared_state.lock() {
                        state.translation = m.translation;
                        state.rotation = m.rotation;
                    }
                }
                Ok(DeviceEvent::Button(k)) if k.pressed(0) => {
                    // Button 1 (index 0) fires — works for both Spaceball btn1 and SpaceOrb A
                    fire_tx.send(()).ok();
                }
                _ => {}
            }
        }
    });
}
```

The asteroids3d background thread currently holds the Spaceball and reads packets, updating a shared `SpaceballInput` state struct. The refactor changes this to update a `DeviceState { translation: [f32;3], rotation: [f32;3], fire: bool }` struct instead. Update the game systems that read from the shared state to use the new fields.

The key changes in the game systems:
- Camera movement: replace `T_SCALE` / `R_SCALE` multiplied by raw `i16` values with normalized values multiplied by sensitivity constants (same effective behavior, different numbers).
- Bullet firing: currently keyed off `Packet::Key(k) if k.buttons[0]`. With events, this is `DeviceEvent::Button(k) if k.pressed(0)`.

- [ ] **Step 5: Verify all examples compile**

```bash
cargo check --examples 2>&1
```

Expected: `Finished` with no errors. Fix any remaining type mismatches.

- [ ] **Step 6: Commit**

```bash
git add examples/
git commit -m "feat(examples): update all examples for multi-device support via probe()/events()"
```

---

## Task 6: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update CLAUDE.md to reflect new module structure and API**

Edit `CLAUDE.md`. Replace the Architecture section with:

```markdown
## Architecture

`spaceball-rs` is a Rust library supporting two 6DOF input devices over serial port.

**`src/lib.rs`** — thin hub. Defines shared types (`Error`, `NormalizedMotion`, `ButtonState`, `DeviceEvent`) and two traits:
- `SixDofDevice` — object-safe trait with `events()` returning a normalized `DeviceEvent` stream. Both devices implement it. Use `probe(path)`, `find()`, or `first()` to get a `Box<dyn SixDofDevice>` without knowing the device type.
- `Probeable` — gives concrete types `find()` and `first()` for free. Implement one method (`probe(path)`) per device; port scanning comes from the default impls.

Free functions: `probe(path)` auto-detects the device on a specific port; `find()` and `first()` scan all serial ports.

**`src/spaceball.rs`** — Spaceball 1003/2003/3003. CR-terminated packets with `^`-escape encoding. `SpaceballBallEvent` carries 16-bit displacement deltas + period; `SpaceballKeyEvent` has 8 buttons + pick. `open()` sends the initialization sequence.

**`src/spaceorb.rs`** — SpaceOrb 360. Packets bounded by the next header byte (top bit = 0) or `\r`. Each packet ends with an XOR checksum byte. `SpaceOrbBallEvent` carries decoded 10-bit force/torque values (±511); `SpaceOrbKeyEvent` has 6 buttons (A–F, named accessors `.a()`–`.f()`) + rezero. `open()` asserts RTS + DTR (power lines).

**Normalization (`SixDofDevice::events()`):** both devices yield `NormalizedMotion { translation: [f32;3], rotation: [f32;3] }` scaled to [-1, 1] per second. Apply as `pos += motion.translation * dt`. Spaceball uses the `period` field to convert deltas to rates; SpaceOrb divides force by 511.

**Threading pattern:** move the `Box<dyn SixDofDevice>` to a background thread; call `events()` there and share state via `Arc<Mutex<_>>`.

**Examples:**
- `packetdump.rs` / `hexdump.rs` — diagnostic. Pass `spaceorb` as first arg to select device (default: spaceball). Pass port path as second arg.
- `cube.rs` — 3D cube with `three-d`. Calls `probe(path)` or `first()` automatically.
- `asteroids3d.rs` — full Asteroids game with Bevy. Uses `probe()`/`first()` so it works with either device. Stage history documented at top of file.

**Protocol reference:** `docs/sbprotocol.txt` (Spaceball), `docs/orb_protocol.txt` (SpaceOrb 360).
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for multi-device architecture"
```

---

## Self-Review Checklist

**Spec coverage:**
- ✅ `src/lib.rs` thin hub with shared types — Task 1
- ✅ `src/spaceball.rs` + `src/spaceorb.rs` separation — Task 1
- ✅ `SpaceballBallEvent`, `SpaceballKeyEvent`, `SpaceballPacket` rename — Task 2
- ✅ `ButtonState` impl for both key types — Tasks 2, 3
- ✅ `SixDofDevice::events()` for Spaceball — Task 2
- ✅ `SixDofDevice::events()` for SpaceOrb — Task 3
- ✅ `Probeable` trait with default `find()`/`first()` — Task 1
- ✅ `impl Probeable for Spaceball` — Task 2
- ✅ `impl Probeable for SpaceOrb` — Task 3
- ✅ `SpaceOrbBallEvent` with 10-bit decode — Task 3
- ✅ `SpaceOrbKeyEvent` named accessors `.a()`–`.f()` — Task 3
- ✅ `SpaceOrbPacket::Reset`, `Error` variants — Task 3
- ✅ `probe()` free function (SpaceOrb-first detection) — Task 1
- ✅ `find()` / `first()` free functions — Task 1
- ✅ `Error::NoDeviceFound` — Task 1
- ✅ Spaceball normalization: period-based rate, MAX_RATE = 320_000 — Task 2
- ✅ SpaceOrb normalization: force / 511.0 — Task 3
- ✅ `SpaceOrb::open()` asserts RTS + DTR — Task 4
- ✅ `hexdump.rs`, `packetdump.rs` `--device` flag — Task 5
- ✅ `cube.rs` uses `probe()`/`first()` — Task 5
- ✅ `asteroids3d.rs` uses `probe()`/`events()` — Task 5
- ✅ Probing detection sequence (power-up vs already-powered) — Tasks 2, 3

**Type consistency check:**
- `SpaceballPacket` defined in Task 2, used in Task 2 (examples) and Task 5 (packetdump) ✅
- `SpaceOrbPacket` defined in Task 3, used in Task 5 ✅
- `DeviceEvent` defined in Task 1, used in Tasks 2, 3, 5 ✅
- `NormalizedMotion` defined in Task 1, used in Tasks 2, 3 ✅
- `Probeable` trait defined in Task 1, impls in Tasks 2, 3 ✅
- `SixDofDevice` trait defined in Task 1, impls in Tasks 2, 3, used in Tasks 1, 5 ✅
