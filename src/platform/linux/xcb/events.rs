use super::XCB;
use crate::{error::OSError, event::*, platform::WindowId};
use x11rb::{connection::Connection, protocol::Event as XEvent, NONE};

pub fn poll_event() -> Result<Option<Event>, OSError> {
    if let Some(event) = XCB.events_queue.lock().pop_back() {
        return Ok(Some(event));
    }
    loop {
        let xevent = XCB.conn.poll_for_event()?;
        if let Some(event) = xevent {
            if let Some(event) = manage_event(event) {
                break Ok(Some(event));
            }
        } else {
            break Ok(None);
        }
    }
}

pub fn run_event_for_queue() -> Result<(), OSError> {
    let xevent = XCB.conn.poll_for_event()?;
    if let Some(event) = xevent.and_then(|e| manage_event(e)) {
        XCB.events_queue.lock().push_front(event);
    }
    Ok(())
}

fn manage_event(event: XEvent) -> Option<Event> {
    match event {
        XEvent::SelectionNotify(e) => {
            XCB.clipboard_receiver_semaphore
                .lock()
                .replace(e.property != NONE);
            None
        }
        _ => try_convert_event(event),
    }
}

fn try_convert_event(xevent: XEvent) -> Option<Event> {
    Some(match xevent {
        XEvent::ClientMessage(event) => {
            let data = event.data.as_data32();
            if event.format == 32 && data[0] == XCB.wm_delete_window {
                return Some(Event::WindowEvent {
                    window: WindowId::from_x11(event.window),
                    event: WindowEvent::CloseRequested,
                });
            }
            return None;
        }
        XEvent::Expose(e) if e.count == 0 => Event::WindowEvent {
            window: WindowId::from_x11(e.window),
            event: WindowEvent::Damaged,
        },
        XEvent::DestroyNotify(e) => Event::WindowEvent {
            window: WindowId::from_x11(e.window),
            event: WindowEvent::Destroy,
        },
        _ => {
            return None;
        }
    })
}
