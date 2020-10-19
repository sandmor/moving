mod wayland;
mod xcb;

use crate::error::OSError;
use crate::{event::Event, window::*};
use mime::Mime;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct WindowId(u32);

impl WindowId {
    fn from_x11(x11: u32) -> Self {
        Self(x11)
    }

    /*fn to_x11(&self) -> u32 {
        self.0
    }*/

    fn from_wayland(wayland: u32) -> Self {
        Self(wayland)
    }

    fn to_wayland(&self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
pub enum WindowPlatformData {
    Xcb(xcb::Window),
    Wayland(wayland::Window),
}

impl WindowPlatformData {
    fn wayland(&self) -> &wayland::Window {
        match self {
            Self::Wayland(ref w) => w,
            _ => unreachable!(),
        }
    }

    fn wayland_mut(&mut self) -> &mut wayland::Window {
        match self {
            Self::Wayland(ref mut w) => w,
            _ => unreachable!(),
        }
    }

    fn xcb(&self) -> &xcb::Window {
        match self {
            Self::Xcb(ref x) => x,
            _ => unreachable!(),
        }
    }

    fn xcb_mut(&mut self) -> &mut xcb::Window {
        match self {
            Self::Xcb(ref mut x) => x,
            _ => unreachable!(),
        }
    }
}

unsafe impl Sync for WindowPlatformData {}
unsafe impl Send for WindowPlatformData {}

pub enum Connection {
    Wayland(wayland::Connection),
    Xcb(xcb::Connection),
}

impl Connection {
    pub fn new() -> Result<Self, OSError> {
        match wayland_client::Display::connect_to_env() {
            Ok(display) => Ok(Self::Wayland(wayland::Connection::from_display(display)?)),
            // The error is not important the fact is that we need to make this work so fallback to X11
            Err(_) => Ok(Self::Xcb(xcb::Connection::new()?)),
        }
    }

    pub fn poll_event(&self) -> Result<Option<Event>, OSError> {
        match self {
            Self::Wayland(wl) => wl.poll_event(),
            Self::Xcb(xcb) => xcb.poll_event(),
        }
    }

    // Clipboard
    pub fn load_from_clipboard(&self, media_type: Mime) -> Result<Option<Vec<u8>>, OSError> {
        match self {
            Self::Wayland(_) => todo!(),
            Self::Xcb(xcb) => xcb.load_from_clipboard(media_type),
        }
    }

    pub fn store_on_clipboard(&self, media_type: mime::Mime, data: &[u8]) -> Result<(), OSError> {
        match self {
            Self::Wayland(_) => todo!(),
            Self::Xcb(xcb) => xcb.store_on_clipboard(media_type, data),
        }
    }
}

#[cfg(feature = "windows")]
impl Connection {
    pub fn create_window(&self, builder: WindowBuilder) -> Result<Window, OSError> {
        match self {
            Self::Wayland(wl) => wl.create_window(builder),
            Self::Xcb(xcb) => xcb.create_window(builder),
        }
    }

    pub fn destroy_window(&self, window: &mut WindowPlatformData) -> Result<(), OSError> {
        match self {
            Self::Wayland(wl) => wl.destroy_window(window.wayland_mut()),
            Self::Xcb(xcb) => xcb.destroy_window(window.xcb_mut()),
        }
    }

    pub fn redraw_window(&self, window: &Window) {
        match self {
            Self::Wayland(wl) => wl.redraw_window(window.platform_data.read().wayland()),
            Self::Xcb(xcb) => xcb.redraw_window(window.platform_data.read().xcb()),
        }
    }
}