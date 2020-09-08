use lazy_static::lazy_static;
use x11rb::{
    protocol::{
        shm::ConnectionExt as ShmConnectionExt,
        xproto::ConnectionExt,
    },
    xcb_ffi::XCBConnection,
};

#[derive(Debug)]
struct XcbInfo {
    conn: XCBConnection,
    screen_num: usize,
    shm: bool, // Is shared memory buffers supported?
    wm_protocols: u32,
    wm_delete_window: u32,
    clipboard: u32,
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
        let shm = conn
            .shm_query_version()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .filter(|reply| reply.shared_pixmaps)
            .is_some();
        XcbInfo { conn, screen_num, wm_protocols, wm_delete_window, clipboard, shm }
    };
}

mod errors;
mod events;
mod window;

pub use self::errors::*;
pub use self::events::*;
pub use self::window::*;