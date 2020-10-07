use crate::{error::OSError, event::*, platform::*, window::*, CONNECTION};
use lazy_static::lazy_static;
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
    windows: RefCell<BTreeMap<WindowId, Arc<RwLock<WindowPlatformData>>>>,
}

impl EventLoop {
    pub fn new() -> EventLoop {
        EventLoop {
            windows: RefCell::new(BTreeMap::new()),
        }
    }

    pub(crate) fn create_window(&self, builder: WindowBuilder) -> Result<Window, OSError> {
        let window = CONNECTION.create_window(builder)?;

        self.windows
            .borrow_mut()
            .insert(window.id, window.platform_data.clone());

        Ok(window)
    }

    pub fn run<H>(&self, mut event_handler: H)
    where
        H: 'static + FnMut(Event, &mut ControlFlow),
    {
        let mut cf = ControlFlow::Poll;
        let mut there_was_an_event_before = false;
        while cf != ControlFlow::Exit {
            let event = CONNECTION.poll_event().unwrap();
            if let Some(event) = event {
                if let Event::WindowEvent { window, ref event } = event {
                    match event {
                        WindowEvent::Destroy => {
                            if let Some(platform_data) = self.windows.borrow_mut().remove(&window) {
                                CONNECTION
                                    .destroy_window(&mut platform_data.write())
                                    .unwrap();
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
        for window in self.windows.borrow().values() {
            CONNECTION.destroy_window(&mut window.write()).unwrap();
        }
    }
}
