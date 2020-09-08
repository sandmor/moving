mod xcb;

use crate::error::OSError;
use crate::{event::Event, window::*};

pub fn poll_event() -> Result<Option<Event>, OSError> {
    xcb::poll_event()
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct WindowId(u32);

impl WindowId {
    pub fn from_x11(id: u32) -> WindowId {
        WindowId(id)
    }
}

#[derive(Debug)]
pub enum WindowPlatform {
    Xcb(xcb::WindowPlatform)
}

pub fn create_window(builder: WindowBuilder) -> Result<Window, OSError>  {
    xcb::create_window(builder)
}

pub fn redraw_window(window: &Window) {
    match *window.platform.read() {
        WindowPlatform::Xcb(ref x) => {
            xcb::redraw_window(window.id, x);
        }
    }
}

pub fn destroy_window(window_platform: &mut WindowPlatform) -> Result<(), OSError> {
    match window_platform {
        WindowPlatform::Xcb(ref mut x) => {
            xcb::destroy_window(x)
        }
    }
}