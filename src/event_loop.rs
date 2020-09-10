use crate::error::OSError;
use crate::event::*;
use crate::platform::*;
use crate::window::*;
use parking_lot::RwLock;
use std::{cell::RefCell, collections::BTreeMap, sync::Arc, thread};

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum ControlFlow {
    Wait,
    Poll,
    Exit,
}

#[derive(Debug)]
pub struct EventLoop {
    windows: RefCell<BTreeMap<WindowId, Arc<RwLock<WindowPlatform>>>>,
}

impl EventLoop {
    pub fn new() -> EventLoop {
        EventLoop {
            windows: RefCell::new(BTreeMap::new()),
        }
    }

    pub(crate) fn create_window(&self, builder: WindowBuilder) -> Result<Window, OSError> {
        let window = create_window(builder)?;

        self.windows
            .borrow_mut()
            .insert(window.id, window.platform.clone());

        Ok(window)
    }

    pub fn run<H>(&self, mut event_handler: H)
    where
        H: 'static + FnMut(Event, &mut ControlFlow),
    {
        let mut cf = ControlFlow::Poll;
        let mut there_was_an_event_before = false;
        while cf != ControlFlow::Exit {
            let event = poll_event().unwrap();
            if let Some(event) = event {
                if let Event::WindowEvent { window, ref event } = event {
                    match event {
                        WindowEvent::Destroy => {
                            if let Some(platform) = self.windows.borrow_mut().remove(&window) {
                                destroy_window(window, &mut platform.write()).unwrap();
                            }
                        }
                        _ => {}
                    }
                }
                event_handler(event, &mut cf);
                there_was_an_event_before = true;
            } else {
                if let ControlFlow::Poll = cf {
                    event_handler(Event::MainEventsCleared, &mut cf);
                } else {
                    if there_was_an_event_before {
                        event_handler(Event::MainEventsCleared, &mut cf);
                    }
                }
                there_was_an_event_before = false;
            }
            thread::yield_now();
        }
    }
}
