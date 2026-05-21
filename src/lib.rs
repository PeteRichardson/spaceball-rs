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

#[derive(Debug)]
pub enum Error {
    Serial(serialport::Error),
    Io(io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Serial(e) => write!(f, "serial port error: {e}"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Serial(e) => Some(e),
            Error::Io(e) => Some(e),
        }
    }
}

impl From<serialport::Error> for Error {
    fn from(e: serialport::Error) -> Self {
        Error::Serial(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl Spaceball {
    /// Open a connection to the Spaceball at the given serial port path,
    /// initialize it, and return a ready-to-read `Spacemouse`.
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

    /// Returns an iterator of decoded [`Packet`]s from the Spaceball.
    pub fn packets(&mut self) -> PacketIter<impl Iterator<Item = Result<u8, io::Error>> + '_> {
        PacketIter {
            inner: self.port.by_ref().bytes(),
        }
    }
}

// ---------------------------------------------------------------------------
// Packet types
// ---------------------------------------------------------------------------

/// A key press/release event. Sent whenever any button state changes.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// True if the pick button (beneath the ball) is pressed.
    pub pick: bool,
    /// States of buttons 1–8; `buttons[0]` is button 1, `buttons[7]` is button 8.
    pub buttons: [bool; 8],
}

/// A ball displacement event. Sent while the ball is in motion.
#[derive(Debug, Clone)]
pub struct BallEvent {
    /// Time since the previous ball event, in units of 1/16 millisecond.
    pub period: u16,
    /// Translation displacement [x, y, z] as signed 16-bit integers.
    pub translation: [i16; 3],
    /// Rotation displacement [x, y, z] as signed 16-bit integers.
    pub rotation: [i16; 3],
}

/// A decoded packet from the Spaceball.
#[derive(Debug)]
pub enum Packet {
    Key(KeyEvent),
    Ball(BallEvent),
    /// Any packet type not specifically handled. Holds the raw decoded bytes
    /// (including the leading type byte, excluding the terminating `\r`).
    Unknown(Vec<u8>),
}

// ---------------------------------------------------------------------------
// PacketIter
// ---------------------------------------------------------------------------

pub struct PacketIter<I> {
    inner: I,
}

impl<I: Iterator<Item = Result<u8, io::Error>>> Iterator for PacketIter<I> {
    type Item = Result<Packet, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut raw: Vec<u8> = Vec::new();

            // Accumulate bytes up to the \r packet terminator, decoding
            // the four binary-mode escape sequences along the way.
            loop {
                match self.inner.next()? {
                    Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                    Err(e) => return Some(Err(e)),
                    Ok(b'\r') => break,
                    Ok(b'^') => match self.inner.next()? {
                        Err(e) => return Some(Err(e)),
                        Ok(b'Q') => raw.push(0x11), // XON
                        Ok(b'S') => raw.push(0x13), // XOFF
                        Ok(b'M') => raw.push(0x0D), // CR
                        Ok(b'^') => raw.push(0x1E), // ^
                        Ok(_) => {}                 // invalid escape, skip
                    },
                    Ok(b) => raw.push(b),
                }
            }

            if !raw.is_empty() {
                return Some(Ok(parse_packet(raw)));
            }
            // bare \r — loop and read the next packet
        }
    }
}

fn parse_packet(raw: Vec<u8>) -> Packet {
    match raw.first() {
        // K packet: K + 2 data bytes
        // byte1: 010<pick><b8><b7><b6><b5>
        // byte2: 0100<b4><b3><b2><b1>
        Some(b'K') if raw.len() == 3 => {
            let b1 = raw[1];
            let b2 = raw[2];
            Packet::Key(KeyEvent {
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
            Packet::Ball(BallEvent {
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
        _ => Packet::Unknown(raw),
    }
}
