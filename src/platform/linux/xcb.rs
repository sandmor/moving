use super::WindowId;
use crate::{
    error::OSError,
    event::*,
    event_loop::ControlFlow,
    rect,
    window::{Window as MWindow, WindowBuilder, WindowInner},
    Size,
};
use lazy_static::lazy_static;
use libc::{mmap, munmap, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::{Mutex, RwLock};
use std::{collections::BTreeMap, os::unix::io::AsRawFd, ptr::null_mut, sync::Arc, thread};
use x11rb::{
    connection::Connection,
    protocol::{
        shm::{self, ConnectionExt as ShmConnectionExt},
        xproto::{self, ConnectionExt},
        Event as XEvent,
    },
    wrapper::ConnectionExt as WrapperConnectionExt,
    xcb_ffi::XCBConnection,
    COPY_DEPTH_FROM_PARENT,
};

lazy_static! {
    static ref CONN: XCBConnection = XCBConnection::connect(None).unwrap().0;
}

#[derive(Debug)]
enum WindowBufferKind {
    Native,
    Shm(shm::Seg),
}

#[derive(Debug)]
pub struct WindowPlatform {
    buffer: &'static mut [u8],
    buffer_kind: WindowBufferKind,
    pixmap: xproto::Pixmap,
    gcontext: xproto::Gcontext,
    width: u16,
    height: u16,
    inner: Arc<RwLock<WindowInner>>,
}

#[derive(Debug)]
pub struct XcbHandle {
    screen_num: usize,
    shm: bool, // Is shared memory buffers supported?
    wm_protocols: u32,
    wm_delete_window: u32,
    windows: Mutex<BTreeMap<WindowId, Arc<RwLock<super::WindowPlatform>>>>,
}

impl XcbHandle {
    pub fn connect() -> Result<XcbHandle, x11rb::errors::ConnectError> {
        let wm_protocols = CONN
            .intern_atom(false, b"WM_PROTOCOLS")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let wm_delete_window = CONN
            .intern_atom(false, b"WM_DELETE_WINDOW")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let shm = CONN
            .shm_query_version()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .filter(|reply| reply.shared_pixmaps)
            .is_some();
        Ok(XcbHandle {
            screen_num: 0,
            shm,
            wm_protocols,
            wm_delete_window,
            windows: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn create_window(&self, builder: WindowBuilder) -> Result<MWindow, OSError> {
        let screen = &CONN.setup().roots[self.screen_num];
        let win_id = CONN.generate_id()?;
        let width = builder.width as u16;
        let height = builder.height as u16;

        let win_aux = xproto::CreateWindowAux::new()
            .win_gravity(xproto::Gravity::NorthWest)
            .event_mask(
                xproto::EventMask::Exposure
                    | xproto::EventMask::StructureNotify
                    | xproto::EventMask::NoEvent,
            );

        CONN.create_window(
            COPY_DEPTH_FROM_PARENT,
            win_id,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            xproto::WindowClass::InputOutput,
            0,
            &win_aux,
        )?;

        CONN.change_property32(
            xproto::PropMode::Replace,
            win_id,
            self.wm_protocols,
            xproto::AtomEnum::ATOM,
            &[self.wm_delete_window],
        )?;

        let gc_aux = xproto::CreateGCAux::new().graphics_exposures(0);
        let gcontext = CONN.generate_id()?;
        CONN.create_gc(gcontext, win_id, &gc_aux)?;

        CONN.map_window(win_id)?;
        CONN.flush()?;

        let pixmap = CONN.generate_id()?;

        let buffer;
        let buffer_kind;

        let frame_buffer_ptr;
        let frame_buffer_len;

        if self.shm {
            let segment_size = (width as u32) * (height as u32) * 4;
            let shmseg = CONN.generate_id()?;
            let reply = CONN
                .shm_create_segment(shmseg, segment_size, false)?
                .reply()
                .unwrap();
            let shm::CreateSegmentReply { shm_fd, .. } = reply;

            let addr = unsafe {
                mmap(
                    null_mut(),
                    segment_size as _,
                    PROT_READ | PROT_WRITE,
                    MAP_SHARED,
                    shm_fd.as_raw_fd(),
                    0,
                )
            };

            if addr == MAP_FAILED {
                CONN.shm_detach(shmseg)?;
                return Err(x11rb::errors::ConnectionError::InsufficientMemory.into());
            }

            frame_buffer_ptr = addr as *mut u8;
            frame_buffer_len = segment_size as usize;
            buffer = unsafe { std::slice::from_raw_parts_mut(frame_buffer_ptr, frame_buffer_len) };

            buffer_kind = WindowBufferKind::Shm(shmseg);

            if let Err(e) =
                CONN.shm_create_pixmap(pixmap, win_id, width, height, screen.root_depth, shmseg, 0)
            {
                let _ = CONN.shm_detach(shmseg);
                return Err(e.into());
            }
        } else {
            CONN.create_pixmap(screen.root_depth, pixmap, win_id, width, height)?;
            todo!();
        }

        let inner = Arc::new(RwLock::new(WindowInner {
            size: Size::new(builder.width, builder.height),
            frame_buffer_ptr,
            frame_buffer_len,
        }));

        let window = Arc::new(RwLock::new(super::WindowPlatform::Xcb(WindowPlatform {
            buffer,
            buffer_kind,
            pixmap,
            gcontext,
            width,
            height,
            inner: inner.clone(),
        })));

        CONN.flush()?;

        self.windows.lock().insert(WindowId(win_id), window.clone());

        Ok(MWindow {
            id: WindowId(win_id),
            inner,
            platform: window
        })
    }

    fn destroy_window(&self, win: &mut WindowPlatform) {
        match win.buffer_kind {
            WindowBufferKind::Native => {}
            WindowBufferKind::Shm(shmseg) => {
                CONN.shm_detach(shmseg).unwrap();
                unsafe {
                    munmap(win.buffer.as_mut_ptr() as *mut _, win.buffer.len());
                }
            }
        }
        CONN.free_pixmap(win.pixmap).unwrap();
    }

    pub fn run<H>(&self, mut event_handler: H)
    where
        H: 'static + FnMut(Event, &mut ControlFlow),
    {
        let mut cf = ControlFlow::Wait;
        let mut there_was_an_event_before = false;
        while cf != ControlFlow::Exit {
            let event = CONN
                .poll_for_event()
                .unwrap()
                .and_then(|x| self.try_convert_event(x));
            if let Some(event) = event {
                if let Event::WindowEvent { window, ref event } = event {
                    match event {
                        WindowEvent::Destroy => {
                            if let Some(platform) = self.windows.lock().remove(&window) {
                                match *platform.write() {
                                    super::WindowPlatform::Xcb(ref mut platform) => {
                                        self.destroy_window(platform);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                event_handler(event, &mut cf);
                there_was_an_event_before = true;
            } else {
                if let ControlFlow::Poll = cf {
                    event_handler(Event::MainEventsCleared, &mut cf);
                } else {
                    if there_was_an_event_before {
                        event_handler(Event::MainEventsCleared, &mut cf);
                    }
                }
                there_was_an_event_before = false;
            }
            thread::yield_now();
        }
        for window in self.windows.lock().values() {
            match *window.write() {
                super::WindowPlatform::Xcb(ref mut window) => {
                    self.destroy_window(window);
                }
            }
        }
    }

    fn try_convert_event(&self, xevent: XEvent) -> Option<Event> {
        Some(match xevent {
            XEvent::ClientMessage(event) => {
                let data = event.data.as_data32();
                if event.format == 32 && data[0] == self.wm_delete_window {
                    return Some(Event::WindowEvent {
                        window: WindowId(event.window),
                        event: WindowEvent::CloseRequested,
                    });
                }
                return None;
            }
            XEvent::Expose(e) if e.count == 0 => Event::WindowEvent {
                window: WindowId(e.window),
                event: WindowEvent::Damaged,
            },
            XEvent::DestroyNotify(e) => Event::WindowEvent {
                window: WindowId(e.window),
                event: WindowEvent::Destroy,
            },
            _ => {
                return None;
            }
        })
    }
}

pub fn redraw_window(id: WindowId, platform: &WindowPlatform) {
    CONN.copy_area(
        platform.pixmap,
        id.0,
        platform.gcontext,
        0,
        0,
        0,
        0,
        platform.width,
        platform.height,
    )
    .unwrap();
    CONN.flush().unwrap();
}
