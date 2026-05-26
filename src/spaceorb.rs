// Placeholder — filled in Task 3.
use crate::{ButtonState, DeviceEvent, Error, Probeable, SixDofDevice};
use std::io;

pub struct SpaceOrb {
    _port: Box<dyn serialport::SerialPort>,
}
pub struct SpaceOrbBallEvent { pub force: [i16; 3], pub torque: [i16; 3] }
pub struct SpaceOrbKeyEvent  { pub rezero: bool, pub buttons: [bool; 6] }
pub enum SpaceOrbPacket {
    Ball(SpaceOrbBallEvent),
    Key(SpaceOrbKeyEvent),
    Reset(String),
    Error { brown_out: bool, eeprom: bool, hardware: bool },
    Unknown(Vec<u8>),
}

impl ButtonState for SpaceOrbKeyEvent {
    fn pressed(&self, i: usize) -> bool { self.buttons.get(i).copied().unwrap_or(false) }
    fn count(&self) -> usize { 6 }
}

impl SixDofDevice for SpaceOrb {
    fn events(&mut self) -> Box<dyn Iterator<Item = Result<DeviceEvent, io::Error>> + '_> {
        Box::new(std::iter::empty())
    }
}

unsafe impl Send for SpaceOrb {}

impl Probeable for SpaceOrb {
    fn probe(_path: &str) -> Result<Self, Error> { Err(Error::NoDeviceFound) }
}
