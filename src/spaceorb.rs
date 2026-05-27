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

/// Parse a SpaceOrb packet from raw bytes: header byte + data bytes (XOR byte already stripped).
pub(crate) fn parse_orb_packet(raw: &[u8]) -> SpaceOrbPacket {
    match raw.first() {
        Some(b'D') if raw.len() == 11 => {
            let data: [u8; 9] = raw[2..11].try_into().unwrap();
            SpaceOrbPacket::Ball(decode_ball_data(&data))
        }
        Some(b'K') if raw.len() == 4 => {
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
pub(crate) fn decode_ball_data(bytes: &[u8; 9]) -> SpaceOrbBallEvent {
    const SPACEWARE: &[u8; 9] = b"SpaceWare";
    let d: [u8; 9] = std::array::from_fn(|i| (bytes[i] ^ SPACEWARE[i]) & 0x7F);

    let fx = ((d[0] as u16) << 3) | ((d[1] as u16) >> 4);
    let fy = (((d[1] & 0x0F) as u16) << 6) | ((d[2] as u16) >> 1);
    let fz = (((d[2] & 0x01) as u16) << 9) | ((d[3] as u16) << 2) | ((d[4] as u16) >> 5);
    let tx = (((d[4] & 0x1F) as u16) << 5) | ((d[5] as u16) >> 2);
    let ty = (((d[5] & 0x03) as u16) << 8) | ((d[6] as u16) << 1) | ((d[7] as u16) >> 6);
    let tz = (((d[7] & 0x3F) as u16) << 4) | ((d[8] as u16) >> 3);

    SpaceOrbBallEvent {
        force:  [sign10(fx), sign10(fy), sign10(fz)],
        torque: [sign10(tx), sign10(ty), sign10(tz)],
    }
}

fn sign10(v: u16) -> i16 {
    let v = v & 0x3FF;
    if v & 0x200 != 0 { (v as i16) | (-1i16 << 10) } else { v as i16 }
}

// ── SpaceOrb struct ──────────────────────────────────────────────────────────

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

        let _ = port.write_request_to_send(true);
        let _ = port.write_data_terminal_ready(true);

        // Drain startup packets (R, !1, !2, \r) — read until silence.
        port.set_timeout(Duration::from_millis(200))?;
        let mut byte = [0u8; 1];
        loop {
            match port.read(&mut byte) {
                Err(e) if e.kind() == io::ErrorKind::TimedOut => break,
                Err(e) => return Err(Error::Io(e)),
                Ok(0) => break,
                Ok(_) => continue,
            }
        }
        port.set_timeout(Duration::from_millis(1000))?;

        Ok(SpaceOrb { port })
    }

    pub fn packets(&mut self) -> SpaceOrbPacketIter<impl io::Read + '_> {
        SpaceOrbPacketIter { inner: &mut *self.port }
    }

    pub fn bytes(&mut self) -> impl Iterator<Item = Result<u8, io::Error>> + '_ {
        use std::io::Read;
        self.port.by_ref().bytes()
    }
}

// Safety: Box<dyn SerialPort> is not Send by default, but the underlying TTYPort is safe to send across threads.
unsafe impl Send for SpaceOrb {}

// ── SpaceOrbPacketIter ────────────────────────────────────────────────────────

pub struct SpaceOrbPacketIter<R> {
    inner: R,
}

impl<R: io::Read> Iterator for SpaceOrbPacketIter<R> {
    type Item = Result<SpaceOrbPacket, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Read header byte (top bit = 0).
            let mut hdr = [0u8; 1];
            loop {
                match self.inner.read(&mut hdr) {
                    Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                    Err(e) => return Some(Err(e)),
                    Ok(0) => return None,
                    Ok(_) => break,
                }
            }

            if hdr[0] == b'\r' {
                continue; // terminator packet — skip
            }
            if hdr[0] & 0x80 != 0 {
                continue; // stray data byte — skip
            }

            let header = hdr[0];

            // Read data bytes (including trailing XOR).
            let data: Vec<u8> = match header {
                b'D' => match read_exact_orb(&mut self.inner, 11) {
                    Some(d) => d,
                    None => return None,
                },
                b'K' => match read_exact_orb(&mut self.inner, 4) {
                    Some(d) => d,
                    None => return None,
                },
                b'E' => match read_exact_orb(&mut self.inner, 3) {
                    Some(d) => d,
                    None => return None,
                },
                b'N' => match read_exact_orb(&mut self.inner, 2) {
                    Some(d) => d,
                    None => return None,
                },
                _ => match read_until_xor(&mut self.inner) {
                    Some(d) => d,
                    None => return None,
                },
            };

            // Build raw = header + data bytes without the trailing XOR byte.
            let mut raw = Vec::with_capacity(1 + data.len());
            raw.push(header);
            if !data.is_empty() {
                raw.extend_from_slice(&data[..data.len().saturating_sub(1)]);
            }

            return Some(Ok(parse_orb_packet(&raw)));
        }
    }
}

fn read_exact_orb<R: io::Read>(r: &mut R, n: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; n];
    let mut pos = 0;
    while pos < n {
        match r.read(&mut buf[pos..]) {
            Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
            Err(_) => return None,
            Ok(0) => return None,
            Ok(got) => pos += got,
        }
    }
    Some(buf)
}

fn read_until_xor<R: io::Read>(r: &mut R) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match r.read(&mut byte) {
            Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
            Err(_) => return None,
            Ok(0) => return None,
            Ok(_) => {
                buf.push(byte[0]);
                if byte[0] & 0x80 != 0 {
                    return Some(buf); // XOR byte — end of packet
                }
            }
        }
    }
}

// ── SixDofDevice ─────────────────────────────────────────────────────────────

impl SixDofDevice for SpaceOrb {
    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_> {
        Box::new(self.packets().filter_map(|pkt| match pkt {
            Err(e) => Some(Err(e)),
            Ok(SpaceOrbPacket::Ball(b)) => {
                Some(Ok(DeviceEvent::Motion(NormalizedMotion {
                    translation: b.force.map(|v| v as f32 / 511.0),
                    rotation:    b.torque.map(|v| v as f32 / 511.0),
                })))
            }
            Ok(SpaceOrbPacket::Key(k)) => {
                Some(Ok(DeviceEvent::Button(Box::new(k))))
            }
            _ => None,
        }))
    }
}

// ── Probeable ────────────────────────────────────────────────────────────────

impl Probeable for SpaceOrb {
    fn probe(path: &str) -> Result<Self, Error> {
        use std::io::{Read, Write};
        use std::time::{Duration, Instant};

        // Short per-read timeout so the deadline loop stays responsive.
        let mut port = serialport::new(path, 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(100))
            .open()?;

        let _ = port.write_request_to_send(true);
        let _ = port.write_data_terminal_ready(true);

        // The SpaceOrb takes ~1 s after power-on before sending its startup
        // sequence: a bare <CR> followed by the 'R' reset packet.  Poll for
        // up to 1.5 s so we don't time out before it speaks.
        let deadline = Instant::now() + Duration::from_millis(1500);
        let mut buf = [0u8; 1];
        loop {
            match port.read(&mut buf) {
                Ok(1) => match buf[0] {
                    b'R' | b'!' => {
                        // SpaceOrb startup packet — confirmed.
                        drop(port);
                        return SpaceOrb::open(path);
                    }
                    b'@' => return Err(Error::NoDeviceFound), // Spaceball
                    _ => {} // skip leading \r and other startup bytes
                },
                _ => {} // read timeout — keep polling until deadline
            }
            if Instant::now() >= deadline {
                break;
            }
        }

        // No startup bytes within 1.5 s — device may already be running.
        // Send a query and check for the '!' response.
        port.write_all(b"?\r")?;
        port.set_timeout(Duration::from_millis(500))?;
        match port.read(&mut buf) {
            Ok(1) if buf[0] == b'!' => {
                drop(port);
                SpaceOrb::open(path)
            }
            _ => Err(Error::NoDeviceFound),
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
        let encoded = spaceware_encode(&pack_10bit([0; 6]));
        let evt = decode_ball_data(&encoded);
        assert_eq!(evt.force,  [0, 0, 0]);
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
    fn decode_negative_one() {
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
    fn decode_all_channels() {
        let vals = [100i16, -100, 200, -200, 300, -300];
        let encoded = spaceware_encode(&pack_10bit(vals));
        let evt = decode_ball_data(&encoded);
        assert_eq!(evt.force,  [100, -100, 200]);
        assert_eq!(evt.torque, [-200, 300, -300]);
    }

    #[test]
    fn parse_key_no_buttons() {
        // K + period(0x80) + status(0x80 = no buttons, top bit always 1) + reserved(0x80)
        let raw = vec![b'K', 0x80, 0x80, 0x80];
        let pkt = parse_orb_packet(&raw);
        if let SpaceOrbPacket::Key(k) = pkt {
            assert!(!k.rezero);
            assert_eq!(k.buttons, [false; 6]);
        } else { panic!("expected Key"); }
    }

    #[test]
    fn parse_key_button_a() {
        // status = 1000_0001 = 0x81 → button A pressed
        let raw = vec![b'K', 0x80, 0x81, 0x80];
        let pkt = parse_orb_packet(&raw);
        if let SpaceOrbPacket::Key(k) = pkt {
            assert!(k.a());
            assert!(!k.b());
        } else { panic!("expected Key"); }
    }

    #[test]
    fn parse_key_rezero() {
        // status = 1100_0000 = 0xC0 → rezero pressed
        let raw = vec![b'K', 0x80, 0xC0, 0x80];
        let pkt = parse_orb_packet(&raw);
        if let SpaceOrbPacket::Key(k) = pkt {
            assert!(k.rezero);
            assert_eq!(k.buttons, [false; 6]);
        } else { panic!("expected Key"); }
    }

    #[test]
    fn parse_reset_packet() {
        let mut raw = vec![b'R'];
        raw.extend_from_slice(b" Spaceball (R) V4.34 19-Oct-96");
        let pkt = parse_orb_packet(&raw);
        if let SpaceOrbPacket::Reset(s) = pkt {
            assert!(s.contains("V4.34"), "got: {s}");
        } else { panic!("expected Reset"); }
    }

    #[test]
    fn button_state_trait() {
        let k = SpaceOrbKeyEvent {
            rezero: false,
            buttons: [true, false, true, false, false, false],
        };
        assert!(k.pressed(0));
        assert!(!k.pressed(1));
        assert_eq!(k.count(), 6);
        assert!(k.any_pressed());
    }

    #[test]
    fn named_button_accessors() {
        let k = SpaceOrbKeyEvent {
            rezero: false,
            buttons: [true, false, false, false, false, true],
        };
        assert!(k.a());
        assert!(!k.b());
        assert!(k.f());
    }

    #[test]
    fn normalization_max_force() {
        let b = SpaceOrbBallEvent { force: [511, 0, 0], torque: [0, 0, 0] };
        let t = b.force[0] as f32 / 511.0;
        assert!((t - 1.0).abs() < 0.01, "expected ~1.0, got {t}");
    }

    #[test]
    fn normalization_min_force() {
        let b = SpaceOrbBallEvent { force: [-511, 0, 0], torque: [0, 0, 0] };
        let t = b.force[0] as f32 / 511.0;
        assert!((t + 1.0).abs() < 0.01, "expected ~-1.0, got {t}");
    }
}
