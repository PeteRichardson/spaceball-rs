# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build the library
cargo build

# Check without building (fast)
cargo check
cargo check --examples

# Run tests
cargo test

# Run an example
cargo run --example packetdump [spaceorb] [/dev/cu.usbserial-...]
cargo run --example hexdump    [spaceorb] [/dev/cu.usbserial-...]
cargo run --example cube       [/dev/cu.usbserial-...]
cargo run --example asteroids3d [/dev/cu.usbserial-...]
```

## Architecture

`spaceball-rs` is a Rust library supporting two 6DOF input devices over serial port.

**`src/lib.rs`** — thin hub. Defines shared types (`Error`, `NormalizedMotion`, `ButtonState`, `DeviceEvent`) and two traits:
- `SixDofDevice` — object-safe trait with `events()` returning a normalized `DeviceEvent` stream. Both devices implement it. Use `probe(path)`, `find()`, or `first()` to get a `Box<dyn SixDofDevice>` without knowing the device type.
- `Probeable` — gives concrete types `find()` and `first()` for free. Implement one method (`probe(path)`) per device; port scanning comes from the default impls.

Free functions: `probe(path)` auto-detects the device on a specific port; `find()` and `first()` scan all serial ports.

**`src/spaceball.rs`** — Spaceball 1003/2003/3003. CR-terminated packets with `^`-escape encoding. `SpaceballBallEvent` carries 16-bit displacement deltas + period; `SpaceballKeyEvent` has 8 buttons + pick. `open()` sends the initialization sequence.

**`src/spaceorb.rs`** — SpaceOrb 360. Packets bounded by the next header byte (top bit = 0) or `\r`. Each packet ends with an XOR checksum byte. `SpaceOrbBallEvent` carries decoded 10-bit force/torque values (±511); `SpaceOrbKeyEvent` has 6 buttons (A–F, named accessors `.a()`–`.f()`) + rezero. `open()` asserts RTS + DTR (power lines).

**Normalization (`SixDofDevice::events()`):** both devices yield `NormalizedMotion { translation: [f32;3], rotation: [f32;3] }` scaled to [-1, 1] per second. Apply as `pos += motion.translation * dt`. Spaceball uses the `period` field to convert deltas to rates; SpaceOrb divides force by 511.

**Threading pattern:** move the `Box<dyn SixDofDevice>` (or concrete device) to a background thread; call `events()` or `packets()` there and share state via `Arc<Mutex<_>>`.

**Examples:**
- `packetdump.rs` / `hexdump.rs` — diagnostic. Pass `spaceorb` as first arg to select device (default: spaceball). Pass port path as second arg.
- `cube.rs` — 3D cube with `three-d`. Calls `probe(path)` or `first()` automatically.
- `asteroids3d.rs` — full Asteroids game with Bevy. Uses `probe()`/`first()` so it works with either device. Stage history documented at top of file.

**Protocol reference:** `docs/sbprotocol.txt` (Spaceball), `docs/orb_protocol.txt` (SpaceOrb 360).
