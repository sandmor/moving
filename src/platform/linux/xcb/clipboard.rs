// X11 has multiples clipboards and called them "selections"

use super::{events::run_event_for_queue, XCB};
use crate::error::OSError;
use mime::Mime;
use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use x11rb::{protocol::xproto::ConnectionExt, CURRENT_TIME, NONE};

pub fn load_from_clipboard(media_type: Mime) -> Result<Option<Vec<u8>>, OSError> {
    let selection_owner = XCB.conn.get_selection_owner(XCB.clipboard)?.reply()?.owner;
    if selection_owner == NONE {
        return Ok(None);
    }
    let conv_target = XCB
        .conn
        .intern_atom(false, media_type.essence_str().as_bytes())?
        .reply()?
        .atom;
    *XCB.clipboard_receiver_semaphore.lock() = None;
    XCB.conn.convert_selection(
        XCB.hidden_window,
        XCB.clipboard,
        conv_target,
        XCB.clipboard_receiver,
        CURRENT_TIME,
    )?;
    let start = Instant::now();
    while Instant::now() < start + Duration::from_millis(2) {
        run_event_for_queue()?;
        if XCB.clipboard_receiver_semaphore.lock().is_some() {
            break;
        }
        thread::yield_now();
    }
    if let Some(conversion_performed) = XCB.clipboard_receiver_semaphore.lock().take() {
        if !conversion_performed {
            return Ok(None); // Conversion could not be performed
        }
    } else {
        return Ok(None); // The selection owner does not give us its data
    }
    let prop = XCB
        .conn
        .get_property(false, XCB.hidden_window, XCB.clipboard_receiver, 0u32, 0, 0)?
        .reply()?;
    if prop.type_ != XCB.incr {
        let prop_length = prop.bytes_after;
        let prop = XCB
            .conn
            .get_property(
                false,
                XCB.hidden_window,
                XCB.clipboard_receiver,
                0u32,
                0,
                prop_length,
            )?
            .reply()?;
        let result = prop.value;
        XCB.conn
            .delete_property(XCB.hidden_window, XCB.clipboard_receiver)?;
        Ok(Some(result))
    } else {
        // The data is received incrementally
        // Start the transference process
        XCB.conn
            .delete_property(XCB.hidden_window, XCB.clipboard_receiver)?;
        let mut data = Vec::new();
        let start = Instant::now();
        XCB.clipboard_data_chunk_received
            .store(false, Ordering::SeqCst);
        loop {
            loop {
                if Instant::now() < start + Duration::from_millis(1) {
                    return Ok(None);
                }
                run_event_for_queue()?;
                if XCB.clipboard_data_chunk_received.load(Ordering::SeqCst) {
                    XCB.clipboard_data_chunk_received
                        .store(false, Ordering::SeqCst);
                    break;
                }
                thread::yield_now();
            }
            let prop = XCB
                .conn
                .get_property(false, XCB.hidden_window, XCB.clipboard_receiver, 0u32, 0, 0)?
                .reply()?;
            let prop_length = prop.bytes_after;
            let prop = XCB
                .conn
                .get_property(
                    true,
                    XCB.hidden_window,
                    XCB.clipboard_receiver,
                    0u32,
                    0,
                    prop_length,
                )?
                .reply()?;
            if prop_length == 0 {
                break;
            }
            data.extend_from_slice(&prop.value);
        }
        Ok(Some(data))
    }
}

pub fn store_on_clipboard(media_type: mime::Mime, data: &[u8]) -> Result<(), OSError> {
    XCB.clipboard_data
        .lock()
        .replace((media_type, data.to_owned()));
    XCB.conn
        .set_selection_owner(XCB.hidden_window, XCB.clipboard, CURRENT_TIME)?;
    Ok(())
}
