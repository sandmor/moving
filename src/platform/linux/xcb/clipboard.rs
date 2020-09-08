// X11 has multiples clipboards and called them "selections"

use super::XCB;
use crate::{clipboard::ClipboardDataKind, error::OSError};
use std::{sync::atomic::Ordering, time::Duration};
use x11rb::{protocol::xproto::ConnectionExt, CURRENT_TIME, NONE};

pub fn load_from_clipboard(kind: ClipboardDataKind) -> Result<Option<Vec<u8>>, OSError> {
    let selection_owner = XCB.conn.get_selection_owner(XCB.clipboard)?.reply()?.owner;
    if selection_owner == NONE {
        return Ok(None);
    }
    *XCB.clipboard_receiver_semaphore.0.lock() = false;
    let conv_target = match kind {
        ClipboardDataKind::Utf8 => XCB.utf8_string,
    };
    XCB.conn.convert_selection(
        XCB.hidden_window,
        XCB.clipboard,
        conv_target,
        XCB.clipboard_receiver,
        CURRENT_TIME,
    )?;
    let &(ref lock, ref cvar) = &*XCB.clipboard_receiver_semaphore;
    let mut lock = lock.lock();
    cvar.wait_for(&mut lock, Duration::from_millis(1000000));
    if !XCB.clipboard_conversion_performed.load(Ordering::SeqCst) {
        return Ok(None); // Conversion could not be performed
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
        unimplemented!("INCR")
    }
}
