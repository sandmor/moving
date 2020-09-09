use super::XCB;
use crate::{error::OSError, event::*, platform::WindowId};
use std::sync::atomic::Ordering;
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{ConnectionExt, PropMode},
        Event as XEvent,
    },
    NONE,
};

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
        XEvent::SelectionClear(_) => {
            *XCB.clipboard_data.lock() = None;
            None
        }
        XEvent::SelectionRequest(e) => {
            if let Some((ref mime, ref data)) = &*XCB.clipboard_data.lock() {
                let atom_name = XCB
                    .conn
                    .intern_atom(false, mime.essence_str().as_bytes())
                    .unwrap()
                    .reply()
                    .unwrap()
                    .atom;
                let valid = e.target == atom_name;
                if valid {
                    XCB.conn
                        .change_property(
                            PropMode::Replace,
                            e.requestor,
                            e.property,
                            atom_name,
                            8,
                            data.len() as u32,
                            &data,
                        )
                        .unwrap();
                }
            }
            None
        }
        XEvent::PropertyNotify(e)
            if e.window == XCB.hidden_window && e.atom == XCB.clipboard_receiver =>
        {
            XCB.clipboard_data_chunk_received
                .store(true, Ordering::SeqCst);
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
