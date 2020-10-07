use super::Connection;
use crate::{error::OSError, event::*, platform::WindowId};
use std::sync::atomic::Ordering;
use x11rb::{connection::Connection as XConnection, protocol::{Event as XEvent, xproto}, NONE};

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
                if e.window == self.hidden_window && e.atom == self.atoms.CLIPBOARD_RECEIVER && e.state == xproto::Property::NewValue =>
            {
                self.clipboard_data_chunk_received
                    .store(true, Ordering::SeqCst);
                None
            }
            _ => self.try_convert_event(event),
        }
    }

    fn try_convert_event(&self, xevent: XEvent) -> Option<Event> {
        Some(match xevent {
            XEvent::ClientMessage(event) => {
                let data = event.data.as_data32();
                if event.format == 32 && data[0] == self.atoms.WM_DELETE_WINDOW {
                    return Some(Event::WindowEvent {
                        window: WindowId(event.window),
                        event: WindowEvent::CloseRequested,
                    });
                }
                return None;
            }
            XEvent::Expose(e) if e.count == 0 => Event::WindowEvent {
                window: WindowId(e.window),
                event: WindowEvent::Damaged,
            },
            XEvent::DestroyNotify(e) => Event::WindowEvent {
                window: WindowId(e.window),
                event: WindowEvent::Destroy,
            },
            _ => {
                return None;
            }
        })
    }

    pub fn run_event_for_queue(&self) -> Result<(), OSError> {
        let xevent = self.conn.poll_for_event()?;
        if let Some(event) = xevent.and_then(|e| self.manage_event(e)) {
            self.events_queue.lock().push_front(event);
        }
        Ok(())
    }
}
