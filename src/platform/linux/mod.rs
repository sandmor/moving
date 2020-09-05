mod xcb;

use xcb::XcbHandle;
use crate::error::{ConnectError, OSError};
use crate::window::*;
use crate::{event_loop::ControlFlow, event::*};

#[derive(Debug)]
pub enum Connection {
    Xcb(XcbHandle),
}

impl Connection {
    pub fn new() -> Result<Connection, ConnectError> {
        let handle = match XcbHandle::connect() {
            Ok(h) => h,
            Err(e) => {
                use x11rb::errors::ConnectError as XConnectError;
                return Err(match e {
                    XConnectError::UnknownError => ConnectError::UnknownError,
                    XConnectError::ParseError => ConnectError::ParseError,
                    XConnectError::InsufficientMemory => ConnectError::InsufficientMemory,
                    XConnectError::DisplayParsingError => ConnectError::UnknownError,
                    XConnectError::InvalidScreen => ConnectError::UnknownError,
                    XConnectError::IOError(e) => ConnectError::IO(e),
                    XConnectError::ZeroIDMask => ConnectError::UnknownError,
                    XConnectError::SetupAuthenticate(_) => ConnectError::Authenticate,
                    XConnectError::SetupFailed(_) => ConnectError::UnknownError,
                });
            }
        };
        Ok(Connection::Xcb(handle))
    }

    pub fn create_window(&self, builder: WindowBuilder) -> Result<Window, OSError> {
        match self {
            Connection::Xcb(handle) => {
                handle.create_window(builder)
            }
        }
    }

    pub fn run<H>(&self, event_handler: H)
    where
        H: 'static + FnMut(Event, &mut ControlFlow),
    {
        match self {
            Connection::Xcb(handle) => {
                handle.run(event_handler)
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct WindowId(u32);

pub fn redraw_window(window: &Window) {
    match *window.platform.read() {
        WindowPlatform::Xcb(ref x) => {
            xcb::redraw_window(window.id, x);
        }
    }
}

#[derive(Debug)]
pub(crate) enum WindowPlatform {
    Xcb(xcb::WindowPlatform)
}