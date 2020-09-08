use x11rb::{connection::Connection, protocol::Event as XEvent};
use super::XCB;
use crate::{error::OSError, event::*, platform::WindowId};

pub fn poll_event() -> Result<Option<Event>, OSError> {
    Ok(XCB.conn.poll_for_event()?.and_then(|x| try_convert_event(x)))
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