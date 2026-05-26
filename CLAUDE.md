# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build the library
cargo build

# Build a specific example
cargo build --example asteroids3d
cargo build --example cube

# Run an example (serial port is optional; falls back to a default USB-serial path)
cargo run --example asteroids3d [/dev/cu.usbserial-...]
cargo run --example packetdump [/dev/cu.usbserial-...]
cargo run --example hexdump    [/dev/cu.usbserial-...]

# Check without building
cargo check

# Run tests (none exist yet)
cargo test
```

## Architecture

`spaceball-rs` is a Rust library for communicating with a Spaceball 6-DOF input device over a serial port. The entire library lives in `src/lib.rs`.

**Library (`src/lib.rs`):**
- `Spaceball::open(path)` — opens the serial port at 9600 8N1, sends the initialization sequence, flushes the echo, and returns a ready-to-use device handle.
- `Spaceball::packets()` — returns a `PacketIter` that yields decoded `Packet` values.
- `Spaceball::bytes()` — raw byte iterator for diagnostics.
- `PacketIter` decodes the binary-mode wire format: packets are CR-terminated, with four `^`-escape sequences for XON/XOFF/CR/`^` embedded in data bytes.
- `Packet` is an enum with `Key(KeyEvent)`, `Ball(BallEvent)`, and `Unknown(Vec<u8>)`.
- `BallEvent` carries `period` (timing), `translation [x,y,z]`, and `rotation [x,y,z]` as signed 16-bit values (raw range ±~16 000 at full deflection).
- `KeyEvent` carries `pick` (bottom button) and `buttons[0..7]` (buttons 1–8).

**Examples:**
- `hexdump.rs` — raw packet bytes in hex, useful for debugging the wire format.
- `packetdump.rs` — decoded `Key`/`Ball`/`Unknown` events printed to stdout.
- `cube.rs` — 3D spinning cube controlled by the Spaceball, rendered with `three-d` + egui.
- `asteroids3d.rs` — full Asteroids game built with Bevy (9 incremental stages documented at the top of the file). Spaceball drives a 6-DOF FPS camera; button 1 fires. Stages add asteroid drift, procedural meshes, splitting, bullets, waves, and wormhole gravity.

**Threading pattern:** The Spaceball is blocking-I/O by nature. The `cube` and `asteroids3d` examples move `Spaceball` onto a background thread behind an `Arc<Mutex<_>>`, reading packets there and sharing state with the render thread.

**Protocol reference:** `docs/sbprotocol.txt` and `docs/SpaceBall_2003-3003_Protocol.pdf` contain the full Spaceball 2003/3003 serial protocol spec.
