use super::Connection;
use crate::{
    dpi::LogicalSize,
    error::OSError,
    platform::{WindowId, WindowPlatformData},
    surface::{self, Surface},
    window as mwin,
};
use atomic::Atomic;
use libc::{mmap, munmap, MAP_ANON, MAP_FAILED, MAP_PRIVATE, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::RwLock;
use std::{
    mem,
    os::unix::io::AsRawFd,
    ptr::null_mut,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Arc,
    },
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
struct MotifHints {
    flags: u64,
    functions: u64,
    decorations: u64,
    input_mode: i64,
    status: u64,
}

#[derive(Debug)]
pub enum WindowBufferKind {
    Native { depth: u8 },
    Shm(shm::Seg),
}

#[derive(Debug)]
pub struct Window {
    buffer_kind: WindowBufferKind,
    pixmap: xproto::Pixmap,
    gcontext: xproto::Gcontext,
    colormap: u32,
    win_id: u32,
    depth: u8,
    pub(super) width: u16,
    pub(super) height: u16,
    shared_surface_data: Arc<(AtomicPtr<u8>, Atomic<surface::SharedData>)>,
}

impl Connection {
    pub fn create_window(&self, builder: mwin::WindowBuilder) -> Result<mwin::Window, OSError> {
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
                    | xproto::EventMask::EnterWindow
                    | xproto::EventMask::LeaveWindow
                    | xproto::EventMask::PointerMotion
                    | xproto::EventMask::ButtonPress
                    | xproto::EventMask::ButtonRelease
                    | xproto::EventMask::KeyPress
                    | xproto::EventMask::KeyRelease,
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

        self.conn.change_property8(
            xproto::PropMode::Replace,
            win_id,
            xproto::AtomEnum::WM_NAME,
            xproto::AtomEnum::STRING,
            builder.title.as_bytes(),
        )?;
        self.conn.change_property8(
            xproto::PropMode::Replace,
            win_id,
            self.atoms._NET_WM_NAME,
            self.atoms.UTF8_STRING,
            builder.title.as_bytes(),
        )?;
        self.conn.change_property32(
            xproto::PropMode::Replace,
            win_id,
            self.atoms.WM_PROTOCOLS,
            xproto::AtomEnum::ATOM,
            &[self.atoms.WM_DELETE_WINDOW],
        )?;
        self.conn.change_property8(
            xproto::PropMode::Replace,
            win_id,
            xproto::AtomEnum::WM_CLIENT_MACHINE,
            self.atoms.STRING,
            // Text encoding in X11 is complicated. Let's use UTF-8 and hope for the best.
            gethostname::gethostname()
                .to_str()
                .unwrap_or("[Invalid]")
                .as_bytes(),
        )?;

        if !builder.decorations {
            let hints = MotifHints {
                flags: 2,
                decorations: 0,
                functions: 0,
                input_mode: 0,
                status: 0,
            };
            let hints =
                unsafe { mem::transmute::<MotifHints, [u8; mem::size_of::<MotifHints>()]>(hints) };
            self.conn.change_property(
                xproto::PropMode::Replace,
                win_id,
                self.atoms._MOTIF_WM_HINTS,
                self.atoms._MOTIF_WM_HINTS,
                32,
                (hints.len() / 4) as u32,
                &hints,
            )?;
        }

        let gc_aux = xproto::CreateGCAux::new().graphics_exposures(0);
        let gcontext = self.conn.generate_id()?;
        self.conn.create_gc(gcontext, win_id, &gc_aux)?;

        self.conn.map_window(win_id)?;
        self.conn.flush()?;

        let (pixmap, buffer_kind, surface) =
            self.create_window_buffer(win_id, depth, width as u32, height as u32)?;

        let logical_size = Arc::new(Atomic::new(LogicalSize {
            w: width as f64,
            h: height as f64,
        }));

        let window = Arc::new(RwLock::new(WindowPlatformData::Xcb(Window {
            buffer_kind,
            pixmap,
            gcontext,
            colormap,
            win_id,
            depth,
            width,
            height,
            shared_surface_data: surface.shared(),
        })));

        self.windows
            .write()
            .insert(WindowId::from_x11(win_id), window.clone());

        Ok(mwin::Window {
            id: WindowId::from_x11(win_id),
            surface,
            logical_size,
            dpi: Arc::new(Atomic::new(1.0)), // TODO: Implement DPI
            platform_data: window,
        })
    }

    pub fn create_window_buffer(
        &self,
        win_id: u32,
        depth: u8,
        width: u32,
        height: u32,
    ) -> Result<(u32, WindowBufferKind, Surface), OSError> {
        let pixmap = self.conn.generate_id()?;

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

            frame_buffer_ptr = addr as *mut u8;
            frame_buffer_len = segment_size as usize;

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

            frame_buffer_ptr = addr as *mut u8;

            buffer_kind = WindowBufferKind::Native { depth };
        }
        let frame_buffer_ptr = AtomicPtr::new(frame_buffer_ptr);
        let shared_data = Atomic::new(surface::SharedData {
            buffer_len: frame_buffer_len,
            width,
            height,
        });

        let shared = Arc::new((frame_buffer_ptr, shared_data));

        self.conn.flush()?;
        Ok((
            pixmap,
            buffer_kind,
            Surface::new(surface::Format::Argb8888, shared),
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
            munmap(
                window.shared_surface_data.0.load(Ordering::SeqCst) as *mut _,
                window
                    .shared_surface_data
                    .1
                    .load(Ordering::SeqCst)
                    .buffer_len,
            );
        }
        self.conn.free_pixmap(window.pixmap)?;
        self.conn.destroy_window(window.win_id)?;
        self.conn.free_colormap(window.colormap)?;
        self.windows
            .write()
            .remove(&WindowId::from_x11(window.win_id));
        Ok(())
    }

    pub fn redraw_window(&self, window: &Window) {
        match window.buffer_kind {
            WindowBufferKind::Native { depth } => {
                let buffer = unsafe {
                    std::slice::from_raw_parts_mut(
                        window.shared_surface_data.0.load(Ordering::SeqCst),
                        window
                            .shared_surface_data
                            .1
                            .load(Ordering::SeqCst)
                            .buffer_len,
                    )
                };
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
                        buffer,
                    )
                    .unwrap();

                self.conn
                    .copy_area(
                        window.pixmap,
                        window.win_id,
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
                        window.win_id,
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
            munmap(
                window.shared_surface_data.0.load(Ordering::SeqCst) as *mut _,
                window
                    .shared_surface_data
                    .1
                    .load(Ordering::SeqCst)
                    .buffer_len,
            );
        }
        self.conn.free_pixmap(window.pixmap)?;
        let (pixmap, buffer_kind, new_surface) = self.create_window_buffer(
            window.win_id,
            window.depth,
            new_width as u32,
            new_height as u32,
        )?;

        window.pixmap = pixmap;
        window.shared_surface_data = new_surface.shared().clone();
        window.buffer_kind = buffer_kind;
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
