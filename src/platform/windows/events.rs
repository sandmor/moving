use super::*;
use crate::{error::*, event::*};
use lazy_static::lazy_static;
use winapi::{
    shared::{
        minwindef::{LPARAM, LRESULT, UINT, WPARAM},
        windef::HWND,
    },
    um::winuser::DefWindowProcW,
};

lazy_static! {
    static ref EVENTS_CHANNEL: (flume::Sender<Event>, flume::Receiver<Event>) = flume::unbounded();
}

impl Connection {
    pub fn poll_event(&self) -> Result<Option<Event>, OSError> {
        Ok(EVENTS_CHANNEL.1.try_recv().ok())
    }
}

pub(super) unsafe extern "system" fn window_proc(
    h_wnd: HWND,
    msg: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if msg == WM_DESTROY {
        //IS_WINDOW_CLOSED = true;

        //PostQuitMessage(0);
        println!("HIT");
        EVENTS_CHANNEL
            .0
            .send(Event::WindowEvent {
                window: WindowId::from_hwnd(h_wnd),
                event: WindowEvent::CloseRequested,
            })
            .unwrap();
    }

    DefWindowProcW(h_wnd, msg, w_param, l_param)
}
