# Multi-Device 6DOF Support: Spaceball + SpaceOrb 360

**Date:** 2026-05-26  
**Status:** Approved

## Goal

Add SpaceOrb 360 support to `spaceball-rs` alongside the existing Spaceball (1003/2003/3003). Callers who know their device get a clean, device-specific API with no cross-device noise. Callers who want device-agnostic 3D navigation get a `probe()` path and a normalized motion stream.

---

## Module Structure

```
src/
  lib.rs        — SixDofDevice, Probeable, NormalizedMotion, ButtonState, DeviceEvent,
                  probe(), find(), first(), Error, re-exports
  spaceball.rs  — Spaceball struct, SpaceballPacket and related types
  spaceorb.rs   — SpaceOrb struct, SpaceOrbPacket and related types
```

Each device module is self-contained: its own `open()`, `impl Probeable`, and `impl SixDofDevice`. `lib.rs` stays thin. Adding a future device type requires only a new module file, `impl Probeable`, `impl SixDofDevice`, and updating the generic `probe()` free function in `lib.rs` by one line.

---

## Shared Types (lib.rs)

```rust
/// Normalized motion rate. Apply as: pos += motion.translation * dt
pub struct NormalizedMotion {
    pub translation: [f32; 3],  // [-1, 1] at max deflection, per second
    pub rotation: [f32; 3],     // [-1, 1] at max deflection, per second
}

/// Generic button access, implemented by both device-specific key event types.
pub trait ButtonState {
    fn pressed(&self, index: usize) -> bool;
    fn count(&self) -> usize;
    fn any_pressed(&self) -> bool { (0..self.count()).any(|i| self.pressed(i)) }
}

/// Device-agnostic event, yielded by SixDofDevice::events().
pub enum DeviceEvent {
    Motion(NormalizedMotion),
    Button(Box<dyn ButtonState + Send>),
}

/// Implemented by both Spaceball and SpaceOrb. Object-safe.
pub trait SixDofDevice: Send {
    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_>;
}

/// Auto-detect the device on `path` and return it as a trait object.
pub fn probe(path: &str) -> Result<Box<dyn SixDofDevice>, Error>;

/// Scan all serial ports; return every recognized device.
pub fn find() -> Vec<Box<dyn SixDofDevice>>;

/// Scan all serial ports; return the first recognized device.
pub fn first() -> Result<Box<dyn SixDofDevice>, Error>;

/// Provides type-specific find() and first() with default impls.
/// Implement probe(path) in each device module; find/first come for free.
pub trait Probeable: Sized + SixDofDevice {
    /// Try to open and identify this specific device type at `path`.
    /// Returns Ok(Self) if confirmed, Err otherwise (wrong device or port failure).
    fn probe(path: &str) -> Result<Self, Error>;

    fn find() -> Vec<Self> {
        serialport::available_ports().unwrap_or_default()
            .into_iter()
            .filter_map(|info| Self::probe(&info.port_name).ok())
            .collect()
    }

    fn first() -> Result<Self, Error> {
        serialport::available_ports().unwrap_or_default()
            .into_iter()
            .find_map(|info| Self::probe(&info.port_name).ok())
            .ok_or(Error::NoDeviceFound)
    }
}

pub enum Error {
    Serial(serialport::Error),
    Io(io::Error),
    NoDeviceFound,
}
```

---

## Spaceball (src/spaceball.rs)

```rust
pub struct Spaceball { /* serial port */ }

pub struct SpaceballBallEvent {
    pub period: u16,            // time since last D packet, in 1/16 ms
    pub translation: [i16; 3], // raw ±~16 000 at full deflection
    pub rotation: [i16; 3],
}

pub struct SpaceballKeyEvent {
    pub pick: bool,
    pub buttons: [bool; 8],    // buttons[0] = button 1, buttons[7] = button 8
}
impl ButtonState for SpaceballKeyEvent { … }

pub enum SpaceballPacket {
    Ball(SpaceballBallEvent),
    Key(SpaceballKeyEvent),
    Unknown(Vec<u8>),
}

impl Spaceball {
    pub fn open(path: &str) -> Result<Self, Error>
    pub fn packets(&mut self) -> impl Iterator<Item = Result<SpaceballPacket, io::Error>> + '_
}
impl SixDofDevice for Spaceball { … }
impl Probeable for Spaceball { fn probe(path: &str) -> Result<Self, Error> { … } }
// Spaceball::find() and Spaceball::first() come from Probeable default impls.
```

Packets are CR-terminated with `^`-escape encoding (unchanged from today). Initialization sequence sent on `open()` is unchanged.

---

## SpaceOrb 360 (src/spaceorb.rs)

```rust
pub struct SpaceOrb { /* serial port */ }

pub struct SpaceOrbBallEvent {
    pub force: [i16; 3],   // decoded 10-bit signed, range ±511
    pub torque: [i16; 3],
}

pub struct SpaceOrbKeyEvent {
    pub rezero: bool,
    pub buttons: [bool; 6],  // [0]=A … [5]=F
}
impl ButtonState for SpaceOrbKeyEvent { … }
// Named accessors: .a() .b() .c() .d() .e() .f()

pub enum SpaceOrbPacket {
    Ball(SpaceOrbBallEvent),
    Key(SpaceOrbKeyEvent),
    Reset(String),           // firmware version string from power-up R packet
    Error { brown_out: bool, eeprom: bool, hardware: bool },
    Unknown(Vec<u8>),
}

impl SpaceOrb {
    pub fn open(path: &str) -> Result<Self, Error>
    pub fn packets(&mut self) -> impl Iterator<Item = Result<SpaceOrbPacket, io::Error>> + '_
}
impl SixDofDevice for SpaceOrb { … }
impl Probeable for SpaceOrb { fn probe(path: &str) -> Result<Self, Error> { … } }
// SpaceOrb::find() and SpaceOrb::first() come from Probeable default impls.
```

**Wire format:** packets terminated by the start of the next packet's header byte (or a standalone `\r`). Each packet ends with an XOR checksum byte (top bit set). The `D` packet's 9 data bytes encode six 10-bit values XOR'd with `"SpaceWare"`.

**Init on `open()`:** assert RTS + DTR (both used for power), then send `?\r` to flush any stale state and confirm the device is alive.

---

## Probing (probe() in lib.rs)

Both devices send a distinct opening byte on power-up:

| Device    | Power-up first byte | Message prefix              |
|-----------|--------------------|-----------------------------|
| Spaceball | `@`                | `@1 Spaceball alive and...` |
| SpaceOrb  | `R`                | `R Spaceball (R) V4.34...`  |

**Detection sequence:**

1. Open port at 9600 8N1. Assert both RTS and DTR.
2. Wait up to 500 ms for an incoming byte.
3. `@` → Spaceball; complete init, return `Box<Spaceball>`.
4. `R` → SpaceOrb; consume reset packet, complete init, return `Box<SpaceOrb>`.
5. Timeout (device already powered): send `?\r`.
   - `!` arrives within 200 ms → SpaceOrb.
   - No response → assume Spaceball.
6. Return `Err` only if the port itself fails — never guess on a silent line without the fallback.

---

## Normalization

`NormalizedMotion` represents rates. Callers apply: `pos += motion.translation * dt`.

**Spaceball:** uses the packet's `period` field (1/16 ms units) to convert displacement to rate, then divides by `MAX_RATE_SB = 320_000` (derived from ±16 000 delta at 20 Hz, where period ≈ 800 units).

```
rate       = delta / (period / 16_000.0)   // raw units per second
normalized = rate / 320_000.0              // ±1 at sustained full deflection
```

If `period` is zero, reuse the previous non-zero period (defensive).

**SpaceOrb:** force/torque values are already instantaneous; normalize directly:

```
normalized = force[i] / 511.0
```

**Buttons** pass through as `DeviceEvent::Button(Box<dyn ButtonState + Send>)`. No normalization — concrete type is erased but `pressed(i)`, `count()`, and `any_pressed()` remain accessible.

---

## Examples

- `hexdump.rs`, `packetdump.rs` — add a required `--device spaceball|spaceorb` CLI argument; default to `spaceball` for backward compatibility. These are diagnostic tools so device-specific packet detail is the point.
- `cube.rs` — update to accept an optional device flag; use `probe()` by default.
- `asteroids3d.rs` — replace `Spaceball`-specific packet handling with `probe()` + `SixDofDevice::events()` so it works with either device.
