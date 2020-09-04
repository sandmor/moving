use crate::error::{ConnectError, OSError};
use crate::event::*;
use crate::platform::*;
use crate::window::*;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum ControlFlow {
    Wait,
    Poll,
    Exit,
}

#[derive(Debug)]
pub struct EventLoop {
    connection: Connection,
}

impl EventLoop {
    pub fn new() -> Result<EventLoop, ConnectError> {
        let connection = Connection::new()?;
        Ok(EventLoop { connection })
    }

    pub(crate) fn create_window(&self, builder: WindowBuilder) -> Result<Window, OSError> {
        self.connection.create_window(builder)
    }

    pub fn run<H>(&self, event_handler: H)
    where
        H: 'static + FnMut(Event, &mut ControlFlow),
    {
        self.connection.run(event_handler)
    }
}
