use crate::error::OSError;
mod clipboard;
mod events;
mod window;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct WindowId(u32);

impl WindowId {
    pub fn from_orbclient(id: u32) -> WindowId {
        WindowId(id)
    }

    pub fn to_orbclient(self) -> Option<u32> {
        Some(self.0)
    }
}

#[derive(Debug)]
pub struct Connection {}

impl Connection {
    pub fn new() -> Result<Self, OSError> {
        Ok(Self {})
    }
}

pub use self::window::*;
