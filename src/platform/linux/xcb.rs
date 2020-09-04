use super::WindowId;
use crate::{
    error::OSError,
    event::*,
    event_loop::ControlFlow,
    rect,
    window::{Window as MWindow, WindowBuilder, WindowInner, WindowToDo},
    Size,
};
use libc::{mmap, munmap, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::{Mutex, RwLock};
use std::{collections::BTreeMap, os::unix::io::AsRawFd, ptr::null_mut, sync::Arc};
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

#[derive(Debug)]
enum WindowBufferKind {
    Native,
    Shm(shm::Seg),
}

#[derive(Debug)]
struct WindowState {
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
    conn: XCBConnection,
    screen_num: usize,
    shm: bool, // Is shared memory buffers supported?
    wm_protocols: u32,
    wm_delete_window: u32,
    windows: Mutex<BTreeMap<WindowId, WindowState>>,
}

impl XcbHandle {
    pub fn connect() -> Result<XcbHandle, x11rb::errors::ConnectError> {
        let (conn, screen_num) = XCBConnection::connect(None)?;
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
        let shm = conn
            .shm_query_version()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .filter(|reply| reply.shared_pixmaps)
            .is_some();
        Ok(XcbHandle {
            conn,
            screen_num,
            shm,
            wm_protocols,
            wm_delete_window,
            windows: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn create_window(&self, builder: WindowBuilder) -> Result<MWindow, OSError> {
        let screen = &self.conn.setup().roots[self.screen_num];
        let win_id = self.conn.generate_id()?;
        let width = builder.width as u16;
        let height = builder.height as u16;

        let win_aux = xproto::CreateWindowAux::new()
            .win_gravity(xproto::Gravity::NorthWest)
            .event_mask(
                xproto::EventMask::Exposure
                    | xproto::EventMask::StructureNotify
                    | xproto::EventMask::NoEvent,
            );

        self.conn.create_window(
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

        self.conn.change_property32(
            xproto::PropMode::Replace,
            win_id,
            self.wm_protocols,
            xproto::AtomEnum::ATOM,
            &[self.wm_delete_window],
        )?;

        let gc_aux = xproto::CreateGCAux::new().graphics_exposures(0);
        let gcontext = self.conn.generate_id()?;
        self.conn.create_gc(gcontext, win_id, &gc_aux)?;

        self.conn.map_window(win_id)?;
        self.conn.flush()?;

        let pixmap = self.conn.generate_id()?;

        let buffer;
        let buffer_kind;

        let frame_buffer_ptr;
        let frame_buffer_len;

        if self.shm {
            let segment_size = (width as u32) * (height as u32) * 4;
            let shmseg = self.conn.generate_id()?;
            let reply = self
                .conn
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
                self.conn.shm_detach(shmseg)?;
                return Err(x11rb::errors::ConnectionError::InsufficientMemory.into());
            }

            frame_buffer_ptr = addr as *mut u8;
            frame_buffer_len = segment_size as usize;
            buffer = unsafe { std::slice::from_raw_parts_mut(frame_buffer_ptr, frame_buffer_len) };

            buffer_kind = WindowBufferKind::Shm(shmseg);

            if let Err(e) = self.conn.shm_create_pixmap(
                pixmap,
                win_id,
                width,
                height,
                screen.root_depth,
                shmseg,
                0,
            ) {
                let _ = self.conn.shm_detach(shmseg);
                return Err(e.into());
            }
        } else {
            self.conn
                .create_pixmap(screen.root_depth, pixmap, win_id, width, height)?;
            todo!();
        }

        let inner = Arc::new(RwLock::new(WindowInner {
            size: Size::new(builder.width, builder.height),
            frame_buffer_ptr,
            frame_buffer_len,
            todo: WindowToDo::empty(),
        }));

        let window = WindowState {
            buffer,
            buffer_kind,
            pixmap,
            gcontext,
            width,
            height,
            inner: inner.clone(),
        };

        self.conn.flush()?;

        self.windows.lock().insert(WindowId(win_id), window);

        Ok(MWindow {
            id: WindowId(win_id),
            inner,
        })
    }

    fn destroy_window(&self, state: &mut WindowState) {
        match state.buffer_kind {
            WindowBufferKind::Native => {}
            WindowBufferKind::Shm(shmseg) => {
                self.conn.shm_detach(shmseg).unwrap();
                unsafe {
                    munmap(state.buffer.as_mut_ptr() as *mut _, state.buffer.len());
                }
            }
        }
        self.conn.free_pixmap(state.pixmap).unwrap();
    }

    pub fn run<H>(&self, mut event_handler: H)
    where
        H: 'static + FnMut(Event, &mut ControlFlow),
    {
        let mut cf = ControlFlow::Wait;
        let mut there_was_an_event_before = false;
        while cf != ControlFlow::Exit {
            for (id, window) in self.windows.lock().iter() {
                let mut todo = window.inner.read().todo;
                if todo.contains(WindowToDo::REDRAW) {
                    self.conn
                        .copy_area(
                            window.pixmap,
                            id.0,
                            window.gcontext,
                            0,
                            0,
                            0,
                            0,
                            window.width,
                            window.height,
                        )
                        .unwrap();
                    todo.remove(WindowToDo::REDRAW);
                }
                window.inner.write().todo = todo;
            }
            self.conn.flush().unwrap();
            let event = self
                .conn
                .poll_for_event()
                .unwrap()
                .and_then(|x| self.try_convert_event(x));
            if let Some(event) = event {
                if let Event::WindowEvent { window, ref event } = event {
                    match event {
                        WindowEvent::Destroy => {
                            if let Some(mut state) = self.windows.lock().remove(&window) {
                                self.destroy_window(&mut state);
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
        }
        for window in self.windows.lock().values_mut() {
            self.destroy_window(window);
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
            XEvent::Expose(e) => Event::WindowEvent {
                window: WindowId(e.window),
                event: WindowEvent::Damaged(
                    rect(e.x as f64, e.y as f64, e.width as f64, e.height as f64),
                    e.count as usize,
                ),
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
