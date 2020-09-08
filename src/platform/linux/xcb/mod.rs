use lazy_static::lazy_static;
use parking_lot::{Condvar, Mutex};
use std::sync::{atomic::AtomicBool, Arc};
use x11rb::{
    connection::Connection,
    protocol::{
        shm::ConnectionExt as ShmConnectionExt,
        xproto::{self, ConnectionExt},
    },
    xcb_ffi::XCBConnection,
    COPY_DEPTH_FROM_PARENT,
};

#[derive(Debug)]
struct XcbInfo {
    conn: XCBConnection,
    screen_num: usize,
    shm: bool, // Is shared memory buffers supported?
    wm_protocols: u32,
    wm_delete_window: u32,
    clipboard: u32,
    utf8_string: u32,
    hidden_window: u32,
    incr: u32,
    clipboard_receiver: u32,
    clipboard_receiver_semaphore: Arc<(Mutex<bool>, Condvar)>,
    clipboard_conversion_performed: AtomicBool,
}

lazy_static! {
    static ref XCB: XcbInfo = {
        let (conn, screen_num) = XCBConnection::connect(None).unwrap();
        let wm_protocols = conn
            .intern_atom(false, b"WM_PROTOCOLS")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let wm_delete_window = conn
            .intern_atom(false, b"WM_DELETE_WINDOW")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let clipboard = conn
            .intern_atom(false, b"CLIPBOARD")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let utf8_string = conn
            .intern_atom(false, b"UTF8_STRING")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let incr = conn
            .intern_atom(false, b"INCR")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let shm = conn
            .shm_query_version()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .filter(|reply| reply.shared_pixmaps)
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
            &xproto::CreateWindowAux::new(),
        )
        .unwrap();

        let clipboard_receiver = conn
            .intern_atom(false, b"CLIPBOARD_RECEIVER")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        XcbInfo {
            conn,
            screen_num,
            wm_protocols,
            wm_delete_window,
            clipboard,
            utf8_string,
            shm,
            hidden_window: win_id,
            incr,
            clipboard_receiver,
            clipboard_receiver_semaphore: Arc::new((Mutex::new(false), Condvar::new())),
            clipboard_conversion_performed: AtomicBool::new(false),
        }
    };
}

impl Drop for XCB {
    fn drop(&mut self) {
        self.conn.destroy_window(self.hidden_window).unwrap();
    }
}

mod clipboard;
mod errors;
mod events;
mod window;

pub use self::clipboard::*;
pub use self::errors::*;
pub use self::events::*;
pub use self::window::*;
