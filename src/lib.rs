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
        // The hardware spec requires CTS to be asserted or the Spaceball won't talk.
        port.write_request_to_send(true)?;

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
}
