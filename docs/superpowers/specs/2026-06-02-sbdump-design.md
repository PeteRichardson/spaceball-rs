# sbdump Design

**Problem:** `packetdump` and `hexdump` are separate tools that require knowing the device type upfront via a manual flag. There is no single diagnostic tool that auto-detects the device, shows wire-level bytes alongside interpreted output, and supports both raw and normalized views. `packetdump` also never received the SpaceOrb auto-detection that `sbprobe` has, making it silently use the wrong parser when a SpaceOrb is connected.

**Goal:** A new example `sbdump` that:
1. Auto-detects the connected device via probe (same order as `sbprobe`: SpaceOrb first, then Spaceball)
2. Prints a time-stamped stream of packets with optional raw hex bytes and/or normalized values
3. Uses a fixed-width line format from day one so Phase 2 (multi-device) can add a device-identifier column without reformatting output

---

## CLI

```
sbdump [PATH] [--spaceorb | --spaceball] [--hex] [-e | --events]
```

| argument | meaning |
|---|---|
| `PATH` | Serial port to probe. If omitted, scans all USB serial ports and uses the first recognized device. |
| `--spaceorb` | Force SpaceOrb mode (skip probe, use `SpaceOrb::open()`). |
| `--spaceball` | Force Spaceball mode (skip probe, use `Spaceball::open()`). |
| `--hex` | Append a hex-bytes column after the device-ID column. |
| `-e`, `--events` | Show normalized `NormalizedMotion` events instead of raw packet values. |

`--spaceorb` and `--spaceball` are mutually exclusive (fatal error if both given).

`--hex` and `--events` are **compatible**: `-e --hex` shows normalized values alongside the raw wire bytes that produced them (feasible because raw bytes originate at the packet layer and are carried through normalization unchanged — see Library Additions below).

**Known gap:** There is no flag to show hex bytes without any parsed output. This can be added later as `--no-parse` if useful; it is explicitly out of scope for Phase 1.

---

## Output Format

### Default (`sbdump`)

*(Examples below show both device types for illustration; Phase 1 is single-device.)*

```
  0.000s  Spaceball  BALL  period=  100  Tr(     0,     0,     0)  R(     0,     0,     0)
  0.123s  Spaceball  KEY   pick=false  buttons=[]
  1.001s  SpaceOrb   BALL  F(    45,   -12,     3)  Tq(     0,     7,   -20)
  1.002s  SpaceOrb   KEY   rezero=false  [A]
  1.003s  SpaceOrb   RESET hello
  1.004s  SpaceOrb   ERR   brown_out=false  eeprom=false  hw=false
  1.005s  SpaceOrb   UNK   40 01 02 03
```

`Tr()` is Spaceball **Tr**anslation; `Tq()` is SpaceOrb **Tq**orque. Both would otherwise appear as `T()` and be easily confused with each other.

`UNK` packets always include their bytes in the parsed column — without them the line conveys nothing useful.

### With `--hex`

```
  0.000s  Spaceball  44 00 00 00  00 00 00 00  00 00 00 00  00 00 00 00  00 00 00 00   BALL  period=  100  Tr(...)
  0.123s  Spaceball  6b 01 00 00  00 00 00 00  00 00 00 00  00 00 00 00  00 00 00 00   KEY   pick=false  buttons=[]
  1.005s  SpaceOrb   40 01 02 03  00 00 00 00  00 00 00 00  00 00 00 00  00 00 00 00   UNK   40 01 02 03
```

Hex column: **fixed 20-byte width**, bytes printed as `xx` in **groups of 4 separated by double spaces**: `xx xx xx xx  xx xx xx xx  ...`. Shorter packets are padded with spaces to the full column width. This is wide enough for the longest packets from either device and keeps the parsed column at a stable offset. Grouping improves scannability without requiring knowledge of packet field boundaries.

**UNK with `--hex`:** both the hex column (raw wire bytes, including `^`-escape sequences and framing for Spaceball) and the parsed column (decoded logical bytes) are shown. They may differ for Spaceball packets due to escaping; for SpaceOrb they will match. The minor redundancy in the SpaceOrb case is accepted for the sake of a consistent rule: the hex column always means raw wire bytes, regardless of packet type.

### With `-e` / `--events`

```
  0.000s  Spaceball  MOTION  Tr( 0.000,  0.000,  0.000)  R( 0.000,  0.000,  0.000)
  0.123s  Spaceball  BUTTON  [. . . . . . . .]
  1.001s  SpaceOrb   MOTION  Tr( 0.044, -0.012,  0.003)  R( 0.000,  0.007, -0.020)
  1.002s  SpaceOrb   BUTTON  [. X . . . .]
```

Events mode maps directly onto `DeviceEvent`: `Motion(NormalizedMotion)` → `MOTION`, `Button(...)` → `BUTTON`. No `RESET`, `ERR`, or `UNK` lines (those don't surface through `events()`).

**Button display in events mode:** `DeviceEvent::Button` holds a `Box<dyn ButtonState>` which exposes only `pressed(index)` and `count()` — device-specific fields like `rezero` (SpaceOrb) and `pick` (Spaceball) are not available. Button output uses a fixed-width dot/`X` pattern with one slot per button in index order: `[. X . . . .]`. This is a deliberate trade-off of the normalized API; use default (packet) mode to see named button fields.

### With `-e --hex`

Same as `--events` but with the hex column inserted between device ID and the event label. Raw bytes come from the packet layer and are carried through `events_with_bytes()` unchanged.

### Time offset

Format: right-aligned seconds with three decimal places, followed by `s`. Field width 8 characters: `%8.3fs`. Resolution: microseconds (Rust `Instant`); displayed at millisecond precision.

### Device ID column

Fixed 9-character width, left-aligned, space-padded. In Phase 1 the value is always `"SpaceOrb "` or `"Spaceball"`.

**Phase 2 note:** When multiple devices are added, this column will hold either the port path (e.g., `/dev/cu.usbserial-AJ03ACPV`) or a user-assigned alias. The column width will be determined at startup from the longest identifier. The line format is otherwise unchanged.

---

## Library Additions

### `RawPacket<P>`

```rust
/// A parsed packet paired with the raw serial bytes that produced it.
pub struct RawPacket<P> {
    /// Raw bytes as received from the serial port, including framing
    /// (CR terminator for Spaceball, checksum byte for SpaceOrb).
    pub raw: Vec<u8>,
    pub packet: P,
}
```

Re-exported from `lib.rs` alongside existing re-exports.

### `packets_with_bytes()` on `Spaceball` and `SpaceOrb`

Concrete methods (not on the `SixDofDevice` trait — trait object safety is not needed here; `sbdump` knows the concrete type after probing).

```rust
// On Spaceball:
pub fn packets_with_bytes(&mut self)
    -> impl Iterator<Item = Result<RawPacket<SpaceballPacket>, io::Error>> + '_

// On SpaceOrb:
pub fn packets_with_bytes(&mut self)
    -> impl Iterator<Item = Result<RawPacket<SpaceOrbPacket>, io::Error>> + '_
```

The implementation collects raw bytes at the same level where the existing packet parsers read them — before unescaping (Spaceball) or before checksum stripping (SpaceOrb).

### `events_with_bytes()` on `Spaceball` and `SpaceOrb`

Built directly on `packets_with_bytes()`. Normalizes each packet to a `DeviceEvent` using the same logic as `events()`, while carrying `raw` forward unchanged.

```rust
// On Spaceball:
pub fn events_with_bytes(&mut self)
    -> impl Iterator<Item = Result<(Vec<u8>, DeviceEvent), io::Error>> + '_

// On SpaceOrb:
pub fn events_with_bytes(&mut self)
    -> impl Iterator<Item = Result<(Vec<u8>, DeviceEvent), io::Error>> + '_
```

---

## sbdump Example Structure

```
examples/sbdump.rs
  fn parse_args() -> Args        // PATH, device override, --hex, -e/--events flags
  fn probe_device(args) -> Device  // SpaceOrb::probe / Spaceball::probe / open for overrides
  fn print_packet_line(t, device_id, raw_opt, label, fields)  // single line formatter
  fn run_packets(device, args)   // loop over packets_with_bytes() or packets()
  fn run_events(device, args)    // loop over events_with_bytes() or events()
  fn main()
```

`print_packet_line` is the single source of truth for the line format. It takes an `Option<&[u8]>` for the hex column so the same function handles `--hex` and non-`--hex` modes.

---

## Error Cases

| situation | output |
|---|---|
| PATH given, nothing responds | `error: no device found at /dev/cu.usbserial-XXXX` → exit 1 |
| PATH omitted, no USB serial ports at all | `error: no serial ports found` → exit 1 |
| PATH omitted, ports found but none recognized | `error: no supported device found (tried N port(s))` → exit 1 |
| `--spaceorb` and `--spaceball` both given | `error: --spaceorb and --spaceball are mutually exclusive` → exit 1 |

---

## Phase 2: Multi-Device (TODO)

**Output atomicity:** With one thread per device all writing to stdout, Rust's `println!` serializes lines via an internal stdout lock, so lines will not interleave mid-line. However, display order is not guaranteed to match packet arrival order when two threads race for the lock — timestamps may appear briefly out of order. If strict chronological output is required, route all packets through a single printer thread via `mpsc::channel`. Deferred to Phase 2.

**Architecture sketch:**
- Parse all PATH arguments at startup; probe each; report any that fail
- Determine max identifier width from all device names/aliases
- Spawn one reader thread per device; each sends `(Instant, String)` (a fully-formatted line) to a shared printer thread via `mpsc::channel`
- Printer thread writes lines to stdout; no locking needed since it is the only writer

**Device naming:**
- Default: the port path (e.g., `/dev/cu.usbserial-AJ03ACPV`)
- Optional: `--name PATH=ALIAS` flag to assign a short alias for cleaner output

---

## Acceptance Criteria

- `sbdump` auto-detects SpaceOrb and Spaceball without flags
- `--spaceorb` / `--spaceball` force the respective device type
- Default output: timestamp + device ID + parsed packet fields
- Spaceball ball uses `Tr()` for translation; SpaceOrb ball uses `Tq()` for torque — no ambiguity
- `UNK` packets always print their bytes in the parsed column
- `--hex`: adds fixed-width 20-byte hex column (groups of 4, double-space separated) between device ID and parsed fields
- `-e` / `--events`: shows `NormalizedMotion` values (`Tr()` / `R()`) instead of raw packet fields
- `-e --hex`: shows both normalized values and hex bytes
- All four flag combinations produce well-aligned, consistent column layout
- Device ID column is fixed-width (9 chars) with space padding
- Clear error messages for all failure cases; non-zero exit on error
- `cargo check --examples` passes
- `packets_with_bytes()` and `events_with_bytes()` added to both device types
- `RawPacket<P>` re-exported from `lib.rs`
