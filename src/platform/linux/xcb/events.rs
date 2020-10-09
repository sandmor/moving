use super::Connection;
use crate::{error::OSError, event::*, platform::WindowId};
use std::sync::atomic::Ordering;
use x11rb::{
    connection::Connection as XConnection,
    protocol::{xproto, Event as XEvent},
    NONE,
};

impl Connection {
    pub fn poll_event(&self) -> Result<Option<Event>, OSError> {
        if let Some(event) = self.events_queue.lock().pop_back() {
            return Ok(Some(event));
        }
        loop {
            let xevent = self.conn.poll_for_event()?;
            if let Some(event) = xevent {
                if let Some(event) = self.manage_event(event) {
                    break Ok(Some(event));
                }
            } else {
                break Ok(None);
            }
        }
    }

    fn manage_event(&self, event: XEvent) -> Option<Event> {
        match event {
            XEvent::ButtonPress(e) => {
                let state = match event {
                    XEvent::ButtonPress(_) => ButtonState::Pressed,
                    XEvent::ButtonRelease(_) => ButtonState::Released,
                    _ => unreachable!()
                };
                let mut immediate = None;
                let mut flag = 1;
                for i in 0..3 {
                    let button = match i {
                        0 => MouseButton::Left,
                        1 => MouseButton::Right,
                        2 => MouseButton::Middle,
                        _ => unreachable!()
                    };
                    if e.detail & flag != 0 {
                        if immediate.is_none() {
                            immediate = Some(button);
                        }
                        else {
                            self.events_queue.lock().push_front(Event::WindowEvent {
                                window: WindowId::from_x11(e.event),
                                event: WindowEvent::MouseButton {
                                    x: e.event_x as _,
                                    y: e.event_y as _,
                                    button,
                                    state
                                },
                            });
                        }
                    }
                    flag <<= 1;
                }
                if let Some(button) = immediate {
                    Some(Event::WindowEvent {
                        window: WindowId::from_x11(e.event),
                        event: WindowEvent::MouseButton {
                            x: e.event_x as _,
                            y: e.event_y as _,
                            button,
                            state
                        },
                    })
                }
                else {
                    None
                }
            },
            XEvent::ConfigureNotify(e) => {
                if let Some(window) = self.windows.read().get(&WindowId::from_x11(e.window)) {
                    let (width, height) = (window.read().xcb().width, window.read().xcb().height);
                    if width != e.width || height != e.height {
                        self.update_win_buffer_size(
                            &mut window.write().xcb_mut(),
                            e.width,
                            e.height,
                        )
                        .unwrap();
                        return Some(Event::WindowEvent {
                            window: WindowId::from_x11(e.window),
                            event: WindowEvent::Resize {
                                width: e.width as f64,
                                height: e.height as f64,
                            },
                        });
                    }
                }
                None
            }
            XEvent::EnterNotify(e) => Some(Event::WindowEvent {
                window: WindowId::from_x11(e.event),
                event: WindowEvent::MouseEnter {
                    x: e.event_x as _,
                    y: e.event_y as _,
                },
            }),
            XEvent::LeaveNotify(e) => Some(Event::WindowEvent {
                window: WindowId::from_x11(e.event),
                event: WindowEvent::MouseEnter {
                    x: e.event_x as _,
                    y: e.event_y as _,
                },
            }),
            XEvent::MotionNotify(e) => Some(Event::WindowEvent {
                window: WindowId::from_x11(e.event),
                event: WindowEvent::MouseMove {
                    x: e.event_x as _,
                    y: e.event_y as _,
                },
            }),
            XEvent::SelectionNotify(e) => {
                self.clipboard_receiver_semaphore
                    .lock()
                    .replace(e.property != NONE);
                None
            }
            XEvent::SelectionRequest(e) => {
                self.process_selection_request(e).unwrap();
                None
            }
            XEvent::PropertyNotify(e)
                if e.window == self.hidden_window
                    && e.atom == self.atoms.CLIPBOARD_RECEIVER
                    && e.state == xproto::Property::NewValue =>
            {
                self.clipboard_data_chunk_received
                    .store(true, Ordering::SeqCst);
                None
            }
            XEvent::ClientMessage(event) => {
                let data = event.data.as_data32();
                if event.format == 32 && data[0] == self.atoms.WM_DELETE_WINDOW {
                    return Some(Event::WindowEvent {
                        window: WindowId::from_x11(event.window),
                        event: WindowEvent::CloseRequested,
                    });
                }
                return None;
            }
            XEvent::Expose(e) if e.count == 0 => Some(Event::WindowEvent {
                window: WindowId::from_x11(e.window),
                event: WindowEvent::Dirted,
            }),
            XEvent::DestroyNotify(e) => Some(Event::WindowEvent {
                window: WindowId::from_x11(e.window),
                event: WindowEvent::Destroy,
            }),
            _ => None,
        }
    }

    pub fn run_event_for_queue(&self) -> Result<(), OSError> {
        let xevent = self.conn.poll_for_event()?;
        if let Some(event) = xevent.and_then(|e| self.manage_event(e)) {
            self.events_queue.lock().push_front(event);
        }
        Ok(())
    }
}
