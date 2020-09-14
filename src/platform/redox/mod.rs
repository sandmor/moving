//use orbclient::Window;
use crate::error::OSError;
use crate::{event::Event, window::*};
use mime::Mime;

pub fn poll_event() -> Result<Option<Event>, OSError> {
    todo!();
}

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
pub enum WindowPlatform {
    //Xcb(xcb::WindowPlatform),
}

pub fn create_window(builder: WindowBuilder) -> Result<Window, OSError> {
    todo!();
    //xcb::create_window(builder)
}

pub fn redraw_window(window: &Window) {
    todo!();
    // match *window.platform.read() {
    //     WindowPlatform::Xcb(ref x) => {
    //         xcb::redraw_window(window.id, x);
    //     }
    // }
}

pub fn destroy_window(
    win_id: WindowId,
    window_platform: &mut WindowPlatform,
) -> Result<(), OSError> {
    todo!();
    // match window_platform {
    //     WindowPlatform::Xcb(ref mut x) => xcb::destroy_window(win_id, x),
    // }
}

pub mod clipboard {
    use super::*;

    pub fn load(media_type: Mime) -> Result<Option<Vec<u8>>, OSError> {
        todo!();
    }

    pub fn store(media_type: mime::Mime, data: &[u8]) -> Result<(), OSError> {
        todo!();
    }
}
