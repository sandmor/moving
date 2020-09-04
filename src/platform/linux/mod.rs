use raw_window_handle::RawWindowHandle;

mod xlib;

use xlib::*;

#[derive(Debug)]
pub enum Handle {
    Xlib(XlibHandle),
}

pub fn handle(handle: RawWindowHandle) -> Handle {
    match handle {
        RawWindowHandle::Xlib(raw) => Handle::Xlib(XlibHandle::new(raw)),
        w @ _ => unimplemented!("{:?}", w),
    }
}

impl Handle {
    pub fn frame_buffer(&mut self) -> &mut [u8] {
        match self {
            Handle::Xlib(xlib) => {
                xlib.frame_buffer()
            }
        }
    }

    pub fn redraw(&mut self) {
        match self {
            Handle::Xlib(xlib) => {
                xlib.redraw()
            }
        }
    }

    pub fn width(&self) -> u32 {
        match self {
            Handle::Xlib(xlib) => {
                xlib.width()
            }
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            Handle::Xlib(xlib) => {
                xlib.height()
            }
        }
    }
}