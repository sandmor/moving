use super::Connection;
use crate::error::OSError;
use mime::Mime;
use std::{
    convert::TryInto,
    mem,
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use x11rb::{
    protocol::xproto::{
        Atom, ConnectionExt, EventMask, PropMode, SelectionNotifyEvent, SelectionRequestEvent,
        Window, SELECTION_NOTIFY_EVENT,
    },
    connection::Connection as XConnection,
    CURRENT_TIME, NONE,
};

impl Connection {
    pub fn load_from_clipboard(&self, media_type: Mime) -> Result<Option<Vec<u8>>, OSError> {
        let selection_owner = self
            .conn
            .get_selection_owner(self.atoms.CLIPBOARD)?
            .reply()?
            .owner;
        if selection_owner == NONE {
            return Ok(None);
        }
        let conv_target = self
            .conn
            .intern_atom(false, media_type.essence_str().as_bytes())?
            .reply()?
            .atom;
        if let Some(result) = self.load_from_clipboard_inner(conv_target)? {
            return Ok(Some(result));
        }
        if media_type.type_() == mime::TEXT && media_type.subtype() == mime::PLAIN {
            let mut utf8 = true;
            if let Some(n) = media_type.get_param("charset") {
                if n.as_str() != "utf-8" {
                    utf8 = false;
                }
            }
            if utf8 {
                for atom in &[self.atoms.UTF8_STRING, self.atoms.STRING, self.atoms.TEXT] {
                    if let Some(result) = self.load_from_clipboard_inner(*atom)? {
                        return Ok(Some(result));
                    }
                }
            }
        }
        Ok(None)
    }

    fn load_from_clipboard_inner(&self, conv_target: Atom) -> Result<Option<Vec<u8>>, OSError> {
        *self.clipboard_receiver_semaphore.lock() = None;
        self.conn.convert_selection(
            self.hidden_window,
            self.atoms.CLIPBOARD,
            conv_target,
            self.atoms.CLIPBOARD_RECEIVER,
            CURRENT_TIME,
        )?;
        self.conn.flush()?;
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(100) {
            self.run_event_for_queue()?;
            if self.clipboard_receiver_semaphore.lock().is_some() {
                break;
            }
            thread::yield_now();
        }
        self.conn.flush()?;
        if let Some(conversion_performed) = self.clipboard_receiver_semaphore.lock().take() {
            if !conversion_performed {
                return Ok(None); // Conversion could not be performed
            }
        } else {
            println!("ERROR");
            return Ok(None); // The selection owner does not give us its data
        }
        let prop = self
            .conn
            .get_property(
                false,
                self.hidden_window,
                self.atoms.CLIPBOARD_RECEIVER,
                0u32,
                0,
                0,
            )?
            .reply()?;
        if prop.type_ != self.atoms.INCR {
            let prop_length = prop.bytes_after;
            let prop = self
                .conn
                .get_property(
                    false,
                    self.hidden_window,
                    self.atoms.CLIPBOARD_RECEIVER,
                    0u32,
                    0,
                    prop_length,
                )?
                .reply()?;
            let result = prop.value;
            self.conn
                .delete_property(self.hidden_window, self.atoms.CLIPBOARD_RECEIVER)?;
            Ok(Some(result))
        } else {
            // The data is received incrementally
            // Start the transference process
            self.clipboard_data_chunk_received
                .store(false, Ordering::SeqCst);
            self.conn
                .delete_property(self.hidden_window, self.atoms.CLIPBOARD_RECEIVER)?;
            self.conn.flush()?;
            let mut data = Vec::new();
            let start = Instant::now();
            loop {
                loop {
                    if start.elapsed() >= Duration::from_millis(100) {
                        return Ok(None);
                    }
                    self.run_event_for_queue()?;
                    if self.clipboard_data_chunk_received.load(Ordering::SeqCst) {
                        break;
                    }
                    thread::yield_now();
                }
                self.clipboard_data_chunk_received.store(false, Ordering::SeqCst);
                let prop = self
                    .conn
                    .get_property(
                        false,
                        self.hidden_window,
                        self.atoms.CLIPBOARD_RECEIVER,
                        0u32,
                        0,
                        0,
                    )?
                    .reply()?;
                let prop_length = prop.bytes_after;
                let prop = self
                    .conn
                    .get_property(
                        true,
                        self.hidden_window,
                        self.atoms.CLIPBOARD_RECEIVER,
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

    pub fn store_on_clipboard(&self, media_type: mime::Mime, data: &[u8]) -> Result<(), OSError> {
        self.clipboard_data
            .lock()
            .insert(media_type, data.to_owned());
        self.conn
            .set_selection_owner(self.hidden_window, self.atoms.CLIPBOARD, CURRENT_TIME)?;
        Ok(())
    }

    pub fn process_selection_request(&self, e: SelectionRequestEvent) -> Result<(), OSError> {
        let requested = String::from_utf8_lossy(
            &self
                .conn
                .get_atom_name(e.target)
                .unwrap()
                .reply()
                .unwrap()
                .name,
        )
        .into_owned();
        println!("{} requested", requested);
        let mut se = SelectionNotifyEvent {
            requestor: e.requestor,
            selection: e.selection,
            target: e.target,
            property: e.property,
            time: e.time,
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
        };
        self.process_selection_request_inner(e.target, e.property, e.requestor, &mut se)?;
        self.conn
            .send_event(true, e.requestor, EventMask::NoEvent, se)?;
        Ok(())
    }

    fn process_selection_request_inner(
        &self,
        target: Atom,
        property: Atom,
        requestor: Window,
        se: &mut SelectionNotifyEvent,
    ) -> Result<(), OSError> {
        if target == self.atoms.TIMESTAMP {
            self.conn.change_property(
                PropMode::Replace,
                requestor,
                property,
                target,
                64,
                1,
                &[0; 8],
            )?;
        } else if target == self.atoms.TARGETS {
            let mut targets = Vec::with_capacity((self.clipboard_data.lock().len() * 4) + 3 + 3);
            targets.extend_from_slice(&self.atoms.TIMESTAMP.to_ne_bytes());
            targets.extend_from_slice(&self.atoms.TARGETS.to_ne_bytes());
            targets.extend_from_slice(&self.atoms.MULTIPLE.to_ne_bytes());
            for mime in self.clipboard_data.lock().keys() {
                let mime_atom = self
                    .conn
                    .intern_atom(false, mime.essence_str().as_bytes())?
                    .reply()?
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
                        targets.extend_from_slice(&self.atoms.UTF8_STRING.to_ne_bytes());
                        targets.extend_from_slice(&self.atoms.MIME_TEXT_PLAIN_UTF8.to_ne_bytes());
                        targets.extend_from_slice(&self.atoms.STRING.to_ne_bytes());
                        targets.extend_from_slice(&self.atoms.TEXT.to_ne_bytes());
                    }
                }
            }
            self.conn.change_property(
                PropMode::Replace,
                requestor,
                property,
                4u32,
                32,
                (targets.len() / 4) as u32,
                &targets,
            )?;
        } else if target == self.atoms.MULTIPLE {
            if property != NONE {
                let prop = self
                    .conn
                    .get_property(false, requestor, property, 0u32, 0, 0)?
                    .reply()?;
                let mut prop_length = prop.bytes_after;
                let a_s = mem::size_of::<Atom>();
                prop_length -= (prop_length as usize % (a_s * 2)) as u32;
                let prop_val = self
                    .conn
                    .get_property(
                        true,
                        self.hidden_window,
                        self.atoms.CLIPBOARD_RECEIVER,
                        0u32,
                        0,
                        prop_length,
                    )?
                    .reply()?
                    .value;
                for i in (0..prop_val.len()).step_by(a_s * 2) {
                    let target = Atom::from_ne_bytes(prop_val[i..i + a_s].try_into().unwrap());
                    let property =
                        Atom::from_ne_bytes(prop_val[i + a_s..i + a_s * 2].try_into().unwrap());
                    if property == NONE {
                        continue;
                    }
                    if target == self.atoms.MULTIPLE {
                        continue;
                    }
                    se.property = property;
                    self.process_selection_request_inner(target, property, requestor, se)?;
                    if se.property == NONE {
                        self.conn.change_property(
                            PropMode::Replace,
                            requestor,
                            property,
                            4u32,
                            32,
                            1,
                            &NONE.to_ne_bytes(),
                        )?;
                    }
                }
                se.target = target;
                self.conn.change_property(
                    PropMode::Replace,
                    requestor,
                    property,
                    4u32,
                    32,
                    1,
                    &self.atoms.NULL.to_ne_bytes(),
                )?;
            }
        } else {
            let mut result = None;
            let mut result_target = target;
            for (mime, data) in self.clipboard_data.lock().iter() {
                let mime_atom = self
                    .conn
                    .intern_atom(false, mime.essence_str().as_bytes())?
                    .reply()?
                    .atom;
                if target == mime_atom {
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
                        if target == self.atoms.MIME_TEXT_PLAIN_UTF8 {
                            result = Some(data.clone());
                            break;
                        } else if target == self.atoms.UTF8_STRING {
                            result = Some(data.clone());
                            break;
                        } else if target == self.atoms.TEXT {
                            result = Some(data.clone());
                            result_target = self.atoms.UTF8_STRING;
                            break;
                        } else if target == self.atoms.STRING {
                            result = Some(data.clone());
                            break;
                        }
                    }
                }
            }
            if let Some(data) = result {
                self.conn.change_property(
                    PropMode::Replace,
                    requestor,
                    property,
                    target,
                    8,
                    data.len() as u32,
                    &data,
                )?;
                se.target = result_target;
            } else {
                se.property = NONE;
            }
        }
        Ok(())
    }
}
