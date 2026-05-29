# sbprobe — Design Spec

**Date:** 2026-05-27  
**Status:** Approved

## Purpose

A diagnostic example (`examples/sbprobe.rs`) that tests and confirms the port-probing logic independently of the game examples. Motivated by the SpaceOrb not being detected by `cube`, `packetdump`, and `asteroids3d`.

## CLI

```bash
cargo run --example sbprobe list
cargo run --example sbprobe watch
```

Subcommand parsed with `std::env::args()`. No new crate dependencies. Unrecognized or missing subcommand prints usage and exits.

## Subcommands

### `list`

1. Call `serialport::available_ports()` to enumerate all serial ports.
2. For each port path, attempt identification in order:
   - `SpaceOrb::probe(&path)` → label `"SpaceOrb"`
   - `Spaceball::probe(&path)` → label `"Spaceball"`
   - Neither succeeds → label `"?"`
3. Print all ports in two-column format and exit:

```
SpaceOrb   /dev/cu.usbserial-ABC12345
Spaceball  /dev/cu.usbserial-AJ03ACPV
?          /dev/cu.usbserial-XYZ99999
```

Showing unrecognized ports alongside identified ones makes it possible to see whether the SpaceOrb's port is visible to the OS but failing identification, vs. not appearing at all.

### `watch`

State: `HashMap<String, &'static str>` mapping port path → label (`"SpaceOrb"`, `"Spaceball"`, or `"?"`).

**Startup:** perform an initial scan (same logic as `list`), populate the map, and print each discovered port as a `+` line.

**Poll loop (every 1 second):**
1. Call `serialport::available_ports()` to get the current port set.
2. For each port not in the map: probe it, add to map, print a `+` line.
3. For each map entry whose path is no longer in the current port set: remove from map, print a `-` line.
4. Sleep 1 second.

If a port disappears and reappears (replug), it is removed on disappearance and re-probed on reappearance — correct behavior for detecting reconnects.

Output format uses elapsed seconds from program start:

```
[  0s] + SpaceOrb   /dev/cu.usbserial-ABC12345
[  0s] + ?          /dev/cu.usbserial-XYZ99999
[  7s] - ?          /dev/cu.usbserial-XYZ99999
[ 12s] + Spaceball  /dev/cu.usbserial-AJ03ACPV
```

Exit with Ctrl-C (default signal handling).

## Implementation notes

- No library changes required. Uses the public `Probeable` trait (`SpaceOrb::probe`, `Spaceball::probe`) already exported from `lib.rs`.
- Probe order matches the free `probe()` function in `lib.rs`: SpaceOrb first (deterministic `?` handshake), then Spaceball.
- Probe calls may block briefly (serial port timeout). With many ports this makes list output slow, but is acceptable for a diagnostic tool.
- Column width for labels: pad to 9 characters (`"SpaceOrb "`, `"Spaceball"`, `"?        "`).
- Elapsed time column: right-align seconds in a 4-char field (`[  0s]`, `[ 12s]`).
