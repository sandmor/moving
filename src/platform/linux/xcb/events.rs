use super::XCB;
use crate::{error::OSError, event::*, platform::WindowId};
use std::sync::atomic::Ordering;
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{ConnectionExt, PropMode, EventMask, SelectionNotifyEvent, SELECTION_NOTIFY_EVENT},
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
        XEvent::SelectionRequest(e) => {
            let requested = String::from_utf8_lossy(&XCB.conn.get_atom_name(e.target).unwrap().reply().unwrap().name).into_owned();
            println!("{} requested", requested);
            if e.target == XCB.atoms.TARGETS {
                let mut targets = Vec::with_capacity((XCB.clipboard_data.lock().len()*4)+3+3);
                targets.extend_from_slice(&XCB.atoms.TIMESTAMP.to_ne_bytes());
                targets.extend_from_slice(&XCB.atoms.TARGETS.to_ne_bytes());
                targets.extend_from_slice(&XCB.atoms.MULTIPLE.to_ne_bytes());
                for mime in XCB.clipboard_data.lock().keys() {
                    let mime_atom = XCB
                        .conn
                        .intern_atom(false, mime.essence_str().as_bytes())
                        .unwrap()
                        .reply()
                        .unwrap()
                        .atom;
                    targets.extend_from_slice(&mime_atom.to_ne_bytes());
                    if mime.type_() == mime::TEXT && mime.subtype() == mime::PLAIN {
                        let mut utf8 = true;
                        if let Some(n) = mime.get_param("charset") {
                            if n.as_str() != "utf-8" {
                                utf8 = false;
                            }
                        }
                        if utf8 {
                            targets.extend_from_slice(&XCB.atoms.UTF8_STRING.to_ne_bytes());
                            targets.extend_from_slice(&XCB.atoms.MIME_TEXT_PLAIN_UTF8.to_ne_bytes());
                            targets.extend_from_slice(&XCB.atoms.STRING.to_ne_bytes());
                            targets.extend_from_slice(&XCB.atoms.TEXT.to_ne_bytes());
                        }
                    }
                }
                let se = SelectionNotifyEvent {
                    requestor: e.requestor,
                    selection: e.selection,
                    target: e.target,
                    property: e.property,
                    time: e.time,
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                };
                XCB.conn
                    .change_property(
                        PropMode::Replace,
                        e.requestor,
                        e.property,
                        4u32,
                        32,
                        (targets.len() / 4) as u32,
                        &targets,
                    )
                    .unwrap();
                XCB.conn.send_event(true, e.requestor, EventMask::NoEvent, se).unwrap();
            }
            else {
                let mut result = None;
                let mut result_target = e.target;
                for (mime, data) in XCB.clipboard_data.lock().iter() {
                    let mime_atom = XCB
                        .conn
                        .intern_atom(false, mime.essence_str().as_bytes())
                        .unwrap()
                        .reply()
                        .unwrap()
                        .atom;
                    if e.target == mime_atom {
                        result = Some(data.clone());
                        break;
                    }
                    if mime.type_() == mime::TEXT && mime.subtype() == mime::PLAIN {
                        let mut utf8 = true;
                        if let Some(n) = mime.get_param("charset") {
                            if n.as_str() != "utf-8" {
                                utf8 = false;
                            }
                        }
                        if utf8 {
                            if e.target == XCB.atoms.MIME_TEXT_PLAIN_UTF8 {
                                result = Some(data.clone());
                                break;
                            }
                            else if e.target == XCB.atoms.UTF8_STRING {
                                result = Some(data.clone());
                                break;
                            }
                            else if e.target == XCB.atoms.TEXT {
                                result = Some(data.clone());
                                result_target = XCB.atoms.UTF8_STRING;
                                break;
                            }
                            else if e.target == XCB.atoms.STRING {
                                result = Some(data.clone());
                                break;
                            }
                        }
                    }
                }
                let mut se = SelectionNotifyEvent {
                    requestor: e.requestor,
                    selection: e.selection,
                    target: result_target,
                    property: NONE,
                    time: e.time,
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                };
                if let Some(data) = result {
                    XCB.conn
                        .change_property(
                            PropMode::Replace,
                            e.requestor,
                            e.property,
                            e.target,
                            8,
                            data.len() as u32,
                            &data,
                        )
                        .unwrap();
                    se.property = e.property;
                }
                XCB.conn.send_event(true, e.requestor, EventMask::NoEvent, se).unwrap();
            }
            None
        }
        XEvent::PropertyNotify(e)
            if e.window == XCB.hidden_window && e.atom == XCB.atoms.CLIPBOARD_RECEIVER =>
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
            if event.format == 32 && data[0] == XCB.atoms.WM_DELETE_WINDOW {
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
