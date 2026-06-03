use crate::{ButtonState, DeviceEvent, Error, NormalizedMotion, Probeable, RawPacket, SixDofDevice};
use std::io::{self, Read};
use std::time::Duration;

// Initialization sequence as specified in the Spaceball protocol:
//   \r          = CR (0x0D) — clear the line
//   CB\r        = Communications Mode Set to Binary
//   NT\r        = Set Null Region to default
//   FT?\r       = Translation Feel (cubic response)
//   FR?\r       = Rotation Feel (cubic response)
//   P@r@r\r     = Data rate to 20 events per second (0x40 0x72 0x40 0x72)
//   MSSV\r      = Ball Event Type: Translation and Rotation Vectors
//   Z\r         = Rezero the ball
//   BcCc\r      = Beep
const INIT_BYTES: &[u8] = b"\rCB\rNT\rFT?\rFR?\rP@r@r\rMSSV\rZ\rBcCc\r";

#[derive(Debug)]
pub struct Spaceball {
    port: Box<dyn serialport::SerialPort>,
}

impl Spaceball {
    /// Open a connection to the Spaceball at the given serial port path,
    /// initialize it, and return a ready-to-read `Spaceball`.
    pub fn open(path: &str) -> Result<Self, Error> {
        let mut port = serialport::new(path, 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(1000))
            .open()?;

        // Assert RTS so the Spaceball's CTS input is driven high.
        // Some USB-serial adapters don't support software modem control and return
        // EINVAL here — ignore the error, as many adapters assert RTS automatically.
        let _ = port.write_request_to_send(true);

        port.write_all(INIT_BYTES)?;
        port.flush()?;

        // The Spaceball echoes init commands back over the wire. Give it a moment
        // to finish, then discard everything in the receive buffer so that
        // subsequent reads only see real ball-event packets.
        std::thread::sleep(Duration::from_millis(200));
        port.clear(serialport::ClearBuffer::Input)?;

        Ok(Spaceball { port })
    }

    /// Returns an iterator over the raw incoming bytes from the Spaceball.
    pub fn bytes(&mut self) -> impl Iterator<Item = Result<u8, io::Error>> + '_ {
        self.port.by_ref().bytes()
    }

    /// Returns an iterator of [`RawPacket`]s from the Spaceball, each carrying
    /// the decoded packet alongside the original wire bytes.
    pub fn packets_with_bytes(
        &mut self,
    ) -> SpaceballRawPacketIter<impl Iterator<Item = Result<u8, io::Error>> + '_> {
        SpaceballRawPacketIter {
            inner: self.port.by_ref().bytes(),
        }
    }

    /// Returns an iterator of decoded [`SpaceballPacket`]s from the Spaceball.
    pub fn packets(
        &mut self,
    ) -> impl Iterator<Item = Result<SpaceballPacket, io::Error>> + '_ {
        self.packets_with_bytes().map(|r| r.map(|rp| rp.packet))
    }
}

// ---------------------------------------------------------------------------
// Packet types
// ---------------------------------------------------------------------------

/// A key press/release event. Sent whenever any button state changes.
#[derive(Debug, Clone)]
pub struct SpaceballKeyEvent {
    /// True if the pick button (beneath the ball) is pressed.
    pub pick: bool,
    /// States of buttons 1–8; `buttons[0]` is button 1, `buttons[7]` is button 8.
    pub buttons: [bool; 8],
}

impl ButtonState for SpaceballKeyEvent {
    fn pressed(&self, i: usize) -> bool {
        self.buttons.get(i).copied().unwrap_or(false)
    }
    fn count(&self) -> usize { 8 }
}

/// A ball displacement event. Sent while the ball is in motion.
#[derive(Debug, Clone)]
pub struct SpaceballBallEvent {
    /// Time since the previous ball event, in units of 1/16 millisecond.
    pub period: u16,
    /// Translation displacement [x, y, z] as signed 16-bit integers.
    pub translation: [i16; 3],
    /// Rotation displacement [x, y, z] as signed 16-bit integers.
    pub rotation: [i16; 3],
}

/// A decoded packet from the Spaceball.
#[derive(Debug)]
pub enum SpaceballPacket {
    Key(SpaceballKeyEvent),
    Ball(SpaceballBallEvent),
    /// Any packet type not specifically handled. Holds the raw decoded bytes
    /// (including the leading type byte, excluding the terminating `\r`).
    Unknown(Vec<u8>),
}

// ---------------------------------------------------------------------------
// SpaceballRawPacketIter
// ---------------------------------------------------------------------------

pub struct SpaceballRawPacketIter<I> {
    inner: I,
}

impl<I: Iterator<Item = Result<u8, io::Error>>> Iterator for SpaceballRawPacketIter<I> {
    type Item = Result<RawPacket<SpaceballPacket>, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut wire: Vec<u8> = Vec::new();
            let mut decoded: Vec<u8> = Vec::new();

            loop {
                match self.inner.next()? {
                    Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                    Err(e) => return Some(Err(e)),
                    Ok(b'\r') => {
                        wire.push(b'\r');
                        break;
                    }
                    Ok(b'^') => {
                        wire.push(b'^');
                        match self.inner.next()? {
                            Err(e) => return Some(Err(e)),
                            Ok(ch) => {
                                wire.push(ch);
                                match ch {
                                    b'Q' => decoded.push(0x11),
                                    b'S' => decoded.push(0x13),
                                    b'M' => decoded.push(0x0D),
                                    b'^' => decoded.push(0x1E),
                                    _ => {} // invalid escape — skip in decoded
                                }
                            }
                        }
                    }
                    Ok(b) => {
                        wire.push(b);
                        decoded.push(b);
                    }
                }
            }

            if !decoded.is_empty() {
                return Some(Ok(RawPacket {
                    raw: wire,
                    packet: parse_packet(decoded),
                }));
            }
            // bare \r — loop and read next packet
        }
    }
}

// ---------------------------------------------------------------------------
// SixDofDevice / Probeable
// ---------------------------------------------------------------------------

// Safety: Box<dyn SerialPort> is not Send by default, but the underlying TTYPort is safe to send across threads.
unsafe impl Send for Spaceball {}

impl SixDofDevice for Spaceball {
    fn device_id(&self) -> &'static str { "Spaceball" }

    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_> {
        let mut last_period = 800u16; // ~50 ms default (20 Hz)
        Box::new(self.packets().filter_map(move |pkt| match pkt {
            Err(e) => Some(Err(e)),
            Ok(SpaceballPacket::Ball(b)) => {
                if b.period > 0 { last_period = b.period; }
                let norm = |v: i16| normalize_spaceball_delta(v, last_period);
                Some(Ok(DeviceEvent::Motion(NormalizedMotion {
                    translation: b.translation.map(norm),
                    rotation: b.rotation.map(norm),
                })))
            }
            Ok(SpaceballPacket::Key(k)) => {
                Some(Ok(DeviceEvent::Button(Box::new(k))))
            }
            Ok(SpaceballPacket::Unknown(_)) => None,
        }))
    }
}

impl Probeable for Spaceball {
    /// Open `path` and confirm a Spaceball is attached.
    ///
    /// Sends `?\r`; if the reply starts with `!` it's a SpaceOrb — return Err.
    /// No reply within 200 ms means assume Spaceball (it ignores `?` quietly).
    fn probe(path: &str) -> Result<Self, Error> {
        let mut port = serialport::new(path, 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(500))
            .open()?;

        let _ = port.write_request_to_send(true);
        // DTR not needed for Spaceball; omitted to match open() behaviour

        // Wait up to 500 ms for a spontaneous power-up byte.
        let mut buf = [0u8; 1];
        match port.read(&mut buf) {
            Ok(1) if buf[0] == b'@' => {
                // Spaceball power-up message — confirmed.
                drop(port); // release before re-opening
                return Spaceball::open(path);
            }
            Ok(1) if buf[0] == b'R' => {
                // SpaceOrb power-up — not a Spaceball.
                return Err(Error::NoDeviceFound);
            }
            _ => {
                // Already powered: send `?\r` — a real Spaceball echoes `?` back.
                // Require that echo; silence or any other response is not a Spaceball.
                port.write_all(b"?\r")?;
                port.set_timeout(Duration::from_millis(500))?;
                match port.read(&mut buf) {
                    Ok(1) if buf[0] == b'?' => {} // Spaceball echo — confirmed
                    Ok(1) if buf[0] == b'!' => return Err(Error::NoDeviceFound), // SpaceOrb
                    _ => return Err(Error::NoDeviceFound), // silence or unexpected
                }
            }
        }

        drop(port); // release before re-opening
        // Confirmed (or assumed) Spaceball — run full initialization.
        Spaceball::open(path)
    }
}

/// Convert a single Spaceball displacement delta to a normalized rate value.
/// `period_units` is in 1/16 ms units (800 = 50 ms = 20 Hz default).
/// Returns a value in [-1.0, 1.0] where 1.0 = full-deflection sustained rate.
pub(crate) fn normalize_spaceball_delta(delta: i16, period_units: u16) -> f32 {
    let period_secs = period_units as f32 / 16_000.0;
    (delta as f32 / period_secs) / 320_000.0
}

pub(crate) fn parse_packet(raw: Vec<u8>) -> SpaceballPacket {
    match raw.first() {
        // K packet: K + 2 data bytes
        // byte1: 010<pick><b8><b7><b6><b5>
        // byte2: 0100<b4><b3><b2><b1>
        Some(b'K') if raw.len() == 3 => {
            let b1 = raw[1];
            let b2 = raw[2];
            SpaceballPacket::Key(SpaceballKeyEvent {
                pick: (b1 & 0x10) != 0,
                buttons: [
                    (b2 & 0x01) != 0, // button 1
                    (b2 & 0x02) != 0, // button 2
                    (b2 & 0x04) != 0, // button 3
                    (b2 & 0x08) != 0, // button 4
                    (b1 & 0x01) != 0, // button 5
                    (b1 & 0x02) != 0, // button 6
                    (b1 & 0x04) != 0, // button 7
                    (b1 & 0x08) != 0, // button 8
                ],
            })
        }
        // D packet: D + 14 data bytes
        // period(u16) tx(i16) ty(i16) tz(i16) rx(i16) ry(i16) rz(i16)
        Some(b'D') if raw.len() == 15 => {
            let d = &raw[1..];
            SpaceballPacket::Ball(SpaceballBallEvent {
                period: u16::from_be_bytes([d[0], d[1]]),
                translation: [
                    i16::from_be_bytes([d[2], d[3]]),
                    i16::from_be_bytes([d[4], d[5]]),
                    i16::from_be_bytes([d[6], d[7]]),
                ],
                rotation: [
                    i16::from_be_bytes([d[8], d[9]]),
                    i16::from_be_bytes([d[10], d[11]]),
                    i16::from_be_bytes([d[12], d[13]]),
                ],
            })
        }
        _ => SpaceballPacket::Unknown(raw),
    }
}

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

    #[test]
    fn normalization_full_deflection_at_20hz() {
        // period=800 (50ms at 20Hz), delta=16000 (full deflection) → ~1.0
        let v = normalize_spaceball_delta(16000, 800);
        assert!((v - 1.0).abs() < 0.01, "expected ~1.0, got {v}");
    }

    #[test]
    fn normalization_negative_deflection() {
        let v = normalize_spaceball_delta(-16000, 800);
        assert!((v + 1.0).abs() < 0.01, "expected ~-1.0, got {v}");
    }

    #[test]
    fn normalization_zero_delta() {
        let v = normalize_spaceball_delta(0, 800);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn device_id_is_spaceball() {
        struct FakeSb;
        impl SixDofDevice for FakeSb {
            fn device_id(&self) -> &'static str { "Spaceball" }
            fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, std::io::Error>> + '_> {
                Box::new(std::iter::empty())
            }
        }
        let d = FakeSb;
        assert_eq!(d.device_id(), "Spaceball");
    }

    #[test]
    fn raw_iter_plain_ball_packet_wire_includes_cr() {
        // D packet with no escape sequences: wire bytes = decoded bytes + CR
        let period: u16 = 800;
        let tx: i16 = 1000;
        let mut wire: Vec<u8> = vec![b'D'];
        wire.extend_from_slice(&period.to_be_bytes());
        wire.extend_from_slice(&tx.to_be_bytes());
        wire.extend_from_slice(&[0u8; 10]); // ty, tz, rx, ry, rz
        wire.push(b'\r');

        let iter = SpaceballRawPacketIter { inner: wire.clone().into_iter().map(Ok) };
        let results: Vec<_> = iter.collect();
        assert_eq!(results.len(), 1);
        let rp = results[0].as_ref().unwrap();
        assert_eq!(rp.raw, wire);
        assert!(matches!(rp.packet, SpaceballPacket::Ball(_)));
    }

    #[test]
    fn raw_iter_escaped_byte_preserved_in_wire() {
        // D packet where period high byte = 0x0D (CR), encoded as ^M on wire
        // Decoded: D 0x0D 0x00 [12 zeros]
        // Wire:    D ^ M  0x00 [12 zeros] CR
        let period_lo: u8 = 0x00;
        let mut wire: Vec<u8> = vec![b'D', b'^', b'M', period_lo];
        wire.extend_from_slice(&[0u8; 12]);
        wire.push(b'\r');

        let iter = SpaceballRawPacketIter { inner: wire.clone().into_iter().map(Ok) };
        let results: Vec<_> = iter.collect();
        assert_eq!(results.len(), 1);
        let rp = results[0].as_ref().unwrap();
        // Wire must include the ^ M escape pair
        assert_eq!(rp.raw, wire);
        // Decoded period high byte must be 0x0D
        if let SpaceballPacket::Ball(b) = &rp.packet {
            assert_eq!((b.period >> 8) as u8, 0x0D);
        } else {
            panic!("expected Ball");
        }
    }

    #[test]
    fn raw_iter_bare_cr_is_skipped() {
        // A bare \r followed by a real K packet
        let key_raw = make_key_raw(false, [false; 8]);
        let mut wire: Vec<u8> = vec![b'\r']; // bare CR
        wire.extend_from_slice(&key_raw);
        wire.push(b'\r');

        let iter = SpaceballRawPacketIter { inner: wire.into_iter().map(Ok) };
        let results: Vec<_> = iter.collect();
        // Only one packet (the K packet), bare CR is skipped
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].as_ref().unwrap().packet, SpaceballPacket::Key(_)));
    }
}
