use super::{WindowId, WindowPlatformData};
use crate::{error::OSError, event::Event};
use mime::Mime;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{atomic::AtomicBool, Arc},
};
use x11rb::{
    atom_manager,
    connection::Connection as XConnection,
    protocol::{
        render::ConnectionExt as XRenderConnectionExt,
        shm::ConnectionExt as ShmConnectionExt,
        xproto::{self, ConnectionExt},
    },
    xcb_ffi::XCBConnection,
    COPY_DEPTH_FROM_PARENT,
};

atom_manager! {
    pub AtomCollection: AtomCollectionCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        CLIPBOARD,
        TARGETS,
        MULTIPLE,
        TIMESTAMP,
        UTF8_STRING,
        TEXT,
        STRING,
        MIME_TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
        INCR,
        CLIPBOARD_RECEIVER,
        NULL,
    }
}

#[derive(Debug)]
pub struct Connection {
    conn: XCBConnection,
    screen_num: usize,
    shm: bool,     // Is shared memory buffers supported?
    xrender: bool, // Is xrender supported(translucent windows)?
    atoms: AtomCollection,
    hidden_window: u32,
    clipboard_receiver_semaphore: Arc<Mutex<Option<bool>>>,
    events_queue: Mutex<VecDeque<Event>>,
    clipboard_data: Mutex<BTreeMap<Mime, Vec<u8>>>,
    clipboard_data_chunk_received: AtomicBool,
    windows: RwLock<BTreeMap<WindowId, Arc<RwLock<WindowPlatformData>>>>,
}

impl Connection {
    pub fn new() -> Result<Self, OSError> {
        let (conn, screen_num) = XCBConnection::connect(None).unwrap();
        let atoms = AtomCollection::new(&conn).unwrap();
        let shm = conn
            .shm_query_version()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .filter(|reply| reply.shared_pixmaps)
            .is_some();
        let xrender = conn
            .render_query_version(7, 5)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some();
        let screen_root = conn.setup().roots[screen_num].root;
        let win_id = conn.generate_id().unwrap();
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            win_id,
            screen_root,
            -10,
            -10,
            1,
            1,
            0,
            xproto::WindowClass::InputOutput,
            0,
            &xproto::CreateWindowAux::new().event_mask(xproto::EventMask::PropertyChange),
        )
        .unwrap();
        let atoms = atoms.reply().unwrap();
        Ok(Self {
            conn,
            screen_num,
            shm,
            xrender,
            atoms,
            hidden_window: win_id,
            clipboard_receiver_semaphore: Arc::new(Mutex::new(None)),
            events_queue: Mutex::new(VecDeque::new()),
            clipboard_data: Mutex::new(BTreeMap::new()),
            clipboard_data_chunk_received: AtomicBool::new(false),
            windows: RwLock::new(BTreeMap::new()),
        })
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.conn.destroy_window(self.hidden_window).unwrap();
    }
}

mod clipboard;
mod errors;
mod events;
mod window;

pub use self::window::*;
