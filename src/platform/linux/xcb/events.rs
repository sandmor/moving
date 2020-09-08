use super::XCB;
use crate::{error::OSError, event::*, platform::WindowId};
use std::sync::atomic::Ordering;
use x11rb::{connection::Connection, protocol::Event as XEvent, NONE};

pub fn poll_event() -> Result<Option<Event>, OSError> {
    let xevent = XCB.conn.poll_for_event()?;
    if let Some(event) = xevent {
        match event {
            XEvent::SelectionNotify(e) => {
                XCB.clipboard_conversion_performed
                    .store(e.property != NONE, Ordering::SeqCst);
                let &(ref lock, ref cvar) = &*XCB.clipboard_receiver_semaphore;
                let mut lock = lock.lock();
                *lock = true;
                cvar.notify_one();
            }
            _ => {}
        }
        Ok(try_convert_event(event))
    } else {
        Ok(None)
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
