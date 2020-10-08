use super::Connection;
use crate::{
    error::OSError,
    platform::{WindowId, WindowPlatformData},
    window as mwin, Size,
};
use libc::{mmap, munmap, MAP_ANON, MAP_FAILED, MAP_PRIVATE, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::RwLock;
use std::{
    os::unix::io::AsRawFd,
    ptr::{null_mut, NonNull},
    sync::Arc,
};
use x11rb::{
    connection::{Connection as XConnection, RequestConnection},
    protocol::{
        render::{self as xrender, ConnectionExt as _, PictType},
        shm::{self, ConnectionExt as _},
        xproto::{self, ColormapAlloc, ConnectionExt, Visualid},
    },
    rust_connection::ReplyError,
    wrapper::ConnectionExt as _,
};

#[derive(Debug)]
pub enum WindowBufferKind {
    Native { depth: u8 },
    Shm(shm::Seg),
}

#[derive(Debug)]
pub struct Window {
    buffer: &'static mut [u8],
    buffer_kind: WindowBufferKind,
    pixmap: xproto::Pixmap,
    gcontext: xproto::Gcontext,
    colormap: u32,
    win_id: u32,
    depth: u8,
    width: u16,
    height: u16,
    pixels_box: Arc<RwLock<mwin::PixelsBox>>,
}

impl Connection {
    pub fn create_window(
        &self,
        builder: mwin::WindowBuilder,
    ) -> Result<
        (
            u32,
            Arc<RwLock<WindowPlatformData>>,
            Arc<RwLock<mwin::PixelsBox>>,
        ),
        OSError,
    > {
        let (depth, visual_id) = self.choose_visual(self.screen_num)?;

        let screen = &self.conn.setup().roots[self.screen_num];
        let win_id = self.conn.generate_id()?;
        let width = builder.width as u16;
        let height = builder.height as u16;

        let colormap = self.conn.generate_id()?;
        self.conn
            .create_colormap(ColormapAlloc::None, colormap, screen.root, visual_id)?;

        let win_aux = xproto::CreateWindowAux::new()
            .win_gravity(xproto::Gravity::NorthWest)
            .background_pixel(x11rb::NONE)
            .border_pixel(screen.black_pixel)
            .colormap(colormap)
            .event_mask(
                xproto::EventMask::Exposure
                    | xproto::EventMask::StructureNotify
                    | xproto::EventMask::NoEvent,
            );

        self.conn.create_window(
            depth,
            win_id,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            xproto::WindowClass::InputOutput,
            visual_id,
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

        let (pixmap, buffer, buffer_kind, pixels_box) =
            self.create_window_buffer(win_id, depth, builder.width, builder.height)?;
        let pixels_box = Arc::new(RwLock::new(pixels_box));

        let window = Arc::new(RwLock::new(WindowPlatformData::Xcb(Window {
            buffer,
            buffer_kind,
            pixmap,
            gcontext,
            colormap,
            win_id,
            depth,
            width,
            height,
            pixels_box: pixels_box.clone(),
        })));

        self.windows
            .write()
            .insert(WindowId::from_x11(win_id), window.clone());

        Ok((win_id, window, pixels_box))
    }

    pub fn create_window_buffer(
        &self,
        win_id: u32,
        depth: u8,
        width: f64,
        height: f64,
    ) -> Result<(u32, &'static mut [u8], WindowBufferKind, mwin::PixelsBox), OSError> {
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
                width as u16,
                height as u16,
                depth,
                shmseg,
                0,
            ) {
                let _ = self.conn.shm_detach(shmseg);
                return Err(e.into());
            }
        } else {
            frame_buffer_len = (width as usize) * (height as usize) * 4;
            self.conn
                .create_pixmap(depth, pixmap, win_id, width as u16, height as u16)?;
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

            buffer_kind = WindowBufferKind::Native { depth };
        }

        self.conn.flush()?;
        Ok((
            pixmap,
            buffer,
            buffer_kind,
            mwin::PixelsBox::from_raw(Size::new(width, height), frame_buffer_ptr, frame_buffer_len),
        ))
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
        self.conn.destroy_window(window.win_id)?;
        self.conn.free_colormap(window.colormap)?;
        Ok(())
    }

    pub fn redraw_window(&self, win_id: u32, window: &Window) {
        match window.buffer_kind {
            WindowBufferKind::Native { depth } => {
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
                        depth,
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

    pub(super) fn update_win_buffer_size(
        &self,
        window: &mut Window,
        new_width: u16,
        new_height: u16,
    ) -> Result<(), OSError> {
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
        let (pixmap, buffer, buffer_kind, pixels_box) = self.create_window_buffer(
            window.win_id,
            window.depth,
            new_width as f64,
            new_height as f64,
        )?;
        window.pixmap = pixmap;
        window.buffer = buffer;
        window.buffer_kind = buffer_kind;
        *window.pixels_box.write() = pixels_box;
        window.width = new_width;
        window.height = new_height;
        Ok(())
    }

    // Next function is take from `x11rb` crate cairo example

    /// Choose a visual to use. This function tries to find a depth=32 visual and falls back to the
    /// screen's default visual.
    fn choose_visual(&self, screen_num: usize) -> Result<(u8, Visualid), ReplyError> {
        let depth = 32;
        let screen = &self.conn.setup().roots[screen_num];

        // Try to use XRender to find a visual with alpha support
        let has_render = self
            .conn
            .extension_information(xrender::X11_EXTENSION_NAME)?
            .is_some();
        if has_render {
            let formats = self.conn.render_query_pict_formats()?.reply()?;
            // Find the ARGB32 format that must be supported.
            let format = formats
                .formats
                .iter()
                .filter(|info| (info.type_, info.depth) == (PictType::Direct, depth))
                .filter(|info| {
                    let d = info.direct;
                    (d.red_mask, d.green_mask, d.blue_mask, d.alpha_mask)
                        == (0xff, 0xff, 0xff, 0xff)
                })
                .find(|info| {
                    let d = info.direct;
                    (d.red_shift, d.green_shift, d.blue_shift, d.alpha_shift) == (16, 8, 0, 24)
                });
            if let Some(format) = format {
                // Now we need to find the visual that corresponds to this format
                if let Some(visual) = formats.screens[screen_num]
                    .depths
                    .iter()
                    .flat_map(|d| &d.visuals)
                    .find(|v| v.format == format.id)
                {
                    return Ok((format.depth, visual.visual));
                }
            }
        }
        Ok((screen.root_depth, screen.root_visual))
    }
}
