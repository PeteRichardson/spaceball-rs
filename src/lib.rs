mod spaceball;
mod spaceorb;
mod input_mode;

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

pub use input_mode::{InputMode, ScaledMotion, process};

// ── SixDofDevice trait ───────────────────────────────────────────────────────

/// Implemented by [`Spaceball`] and [`SpaceOrb`]. Object-safe.
///
/// Use [`probe`], [`find`], or [`first`] to obtain a `Box<dyn SixDofDevice>`
/// without knowing which device is attached.
pub trait SixDofDevice: Send {
    /// Short human-readable device type name, e.g. `"Spaceball"` or `"SpaceOrb"`.
    /// Used as the device-ID column in dump output and probe listings.
    fn device_id(&self) -> &'static str;

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
        candidate_ports()
            .into_iter()
            .filter_map(|info| Self::probe(&info.port_name).ok())
            .collect()
    }

    /// Scan all serial ports and return the first device of this type found.
    fn first() -> Result<Self, Error> {
        candidate_ports()
            .into_iter()
            .find_map(|info| Self::probe(&info.port_name).ok())
            .ok_or(Error::NoDeviceFound)
    }
}

// ── Port scanning helper ──────────────────────────────────────────────────────

/// Serial ports worth probing for 6DOF devices.
///
/// Two filters applied:
/// - Skip `/dev/tty.*` on macOS: every physical port appears as both
///   `/dev/cu.NAME` (call-out) and `/dev/tty.NAME` (dial-in, blocks on DCD).
///   The dot distinguishes macOS `tty.NAME` from Linux `ttyUSB0` / `ttyACM0`,
///   which must not be filtered.
/// - Skip non-USB ports: Spaceball and SpaceOrb connect via USB-serial adapters;
///   Bluetooth, PCI, and unknown ports are never the right target.
fn candidate_ports() -> Vec<serialport::SerialPortInfo> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            !p.port_name.contains("/dev/tty.")
                && matches!(p.port_type, serialport::SerialPortType::UsbPort(_))
        })
        .collect()
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
    candidate_ports()
        .into_iter()
        .filter_map(|info| probe(&info.port_name).ok())
        .collect()
}

/// Scan all serial ports and return the first recognized 6DOF device.
pub fn first() -> Result<Box<dyn SixDofDevice>, Error> {
    candidate_ports()
        .into_iter()
        .find_map(|info| probe(&info.port_name).ok())
        .ok_or(Error::NoDeviceFound)
}
