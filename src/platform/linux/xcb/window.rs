use super::Connection;
use crate::{error::OSError, window as mwin, Size};
use libc::{mmap, munmap, MAP_ANON, MAP_FAILED, MAP_PRIVATE, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::RwLock;
use std::{
    os::unix::io::AsRawFd,
    ptr::{null_mut, NonNull},
    sync::Arc,
};
use x11rb::{
    connection::Connection as XConnection,
    protocol::{
        shm::{self, ConnectionExt as ShmConnectionExt},
        xproto::{self, ConnectionExt},
    },
    wrapper::ConnectionExt as WrapperConnectionExt,
    COPY_DEPTH_FROM_PARENT,
};

#[derive(Debug)]
pub enum WindowBufferKind {
    Native { screen_depth: u8 },
    Shm(shm::Seg),
}

#[derive(Debug)]
pub struct Window {
    buffer: &'static mut [u8],
    buffer_kind: WindowBufferKind,
    pixmap: xproto::Pixmap,
    gcontext: xproto::Gcontext,
    win_id: u32,
    width: u16,
    height: u16,
    pixels_box: Arc<RwLock<mwin::PixelsBox>>,
}

impl Connection {
    pub fn create_window(
        &self,
        builder: mwin::WindowBuilder,
    ) -> Result<(u32, Window, Arc<RwLock<mwin::PixelsBox>>), OSError> {
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
            self.atoms.WM_PROTOCOLS,
            xproto::AtomEnum::ATOM,
            &[self.atoms.WM_DELETE_WINDOW],
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
                .reply()?;
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

            frame_buffer_ptr = NonNull::new(addr as *mut u8).unwrap();
            frame_buffer_len = segment_size as usize;
            buffer = unsafe {
                std::slice::from_raw_parts_mut(frame_buffer_ptr.as_ptr(), frame_buffer_len)
            };

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
            frame_buffer_len = (width as usize) * (height as usize) * 4;
            self.conn
                .create_pixmap(screen.root_depth, pixmap, win_id, width, height)?;
            let addr = unsafe {
                mmap(
                    null_mut(),
                    frame_buffer_len,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANON,
                    0,
                    0,
                )
            };

            if addr == MAP_FAILED {
                return Err(x11rb::errors::ConnectionError::InsufficientMemory.into());
            }

            frame_buffer_ptr = NonNull::new(addr as *mut u8).unwrap();

            buffer = unsafe {
                std::slice::from_raw_parts_mut(frame_buffer_ptr.as_ptr(), frame_buffer_len)
            };

            buffer_kind = WindowBufferKind::Native {
                screen_depth: screen.root_depth,
            };
        }

        self.conn.flush()?;

        let pixels_box = Arc::new(RwLock::new(mwin::PixelsBox::from_raw(
            Size::new(builder.width, builder.height),
            frame_buffer_ptr,
            frame_buffer_len,
        )));

        let window = Window {
            buffer,
            buffer_kind,
            pixmap,
            gcontext,
            win_id,
            width,
            height,
            pixels_box: pixels_box.clone(),
        };
        Ok((win_id, window, pixels_box))
    }

    pub fn destroy_window(&self, window: &mut Window) -> Result<(), OSError> {
        match window.buffer_kind {
            WindowBufferKind::Native { .. } => {}
            WindowBufferKind::Shm(shmseg) => {
                self.conn.shm_detach(shmseg)?;
            }
        }
        unsafe {
            munmap(window.buffer.as_mut_ptr() as *mut _, window.buffer.len());
        }
        self.conn.free_pixmap(window.pixmap)?;
        self.conn.destroy_window(window.win_id).unwrap();
        Ok(())
    }

    pub fn redraw_window(&self, win_id: u32, window: &Window) {
        match window.buffer_kind {
            WindowBufferKind::Native { screen_depth } => {
                self.conn
                    .put_image(
                        xproto::ImageFormat::ZPixmap,
                        window.pixmap,
                        window.gcontext,
                        window.width,
                        window.height,
                        0,
                        0,
                        0,
                        screen_depth,
                        window.buffer,
                    )
                    .unwrap();

                self.conn
                    .copy_area(
                        window.pixmap,
                        win_id,
                        //screen_gcontext,
                        window.gcontext,
                        0,
                        0,
                        0,
                        0,
                        window.width,
                        window.height,
                    )
                    .unwrap();
            }
            WindowBufferKind::Shm(_) => {
                self.conn
                    .copy_area(
                        window.pixmap,
                        win_id,
                        window.gcontext,
                        0,
                        0,
                        0,
                        0,
                        window.width,
                        window.height,
                    )
                    .unwrap();
            }
        }
        self.conn.flush().unwrap();
    }
}
