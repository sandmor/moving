use parking_lot::RwLock;
use std::{collections::BTreeMap, sync::Arc};
use winapi::{shared::windef::HWND, um::winuser::WM_DESTROY};

use crate::{error::OSError, event::Event};
mod clipboard;
mod events;
mod window;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct WindowId(HWND);

impl WindowId {
    fn from_hwnd(hwnd: HWND) -> Self {
        Self(hwnd)
    }

    fn to_hwnd(&self) -> HWND {
        self.0
    }
}

unsafe impl Sync for WindowId {}
unsafe impl Send for WindowId {}

#[derive(Debug)]
pub struct Connection {
    windows: RwLock<BTreeMap<WindowId, Arc<RwLock<WindowPlatformData>>>>,
}

impl Connection {
    pub fn new() -> Result<Self, OSError> {
        Ok(Self {
            windows: RwLock::new(BTreeMap::new()),
        })
    }
}

pub use self::window::*;
