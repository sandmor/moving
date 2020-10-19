mod frame;
use frame::*;

use super::Connection;
use crate::{
    dpi::LogicalSize,
    error::OSError,
    event::*,
    platform::{WindowId, WindowPlatformData},
    surface::{self, Surface},
    window as mwin,
};
use atomic::Atomic;
use libc::{mmap, munmap, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::RwLock;
use std::{
    os::unix::io::AsRawFd,
    ptr::null_mut,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Arc,
    },
};
use wayland_client::{
    protocol::{wl_shm::Format, wl_surface::WlSurface},
    Main,
};
use wayland_protocols::xdg_shell::client::xdg_toplevel::XdgToplevel;

#[derive(Debug)]
pub struct Window {
    xdg_toplevel: Main<XdgToplevel>,
    buf_x: i32,
    buf_y: i32,
    surface: Surface,
    wl_surface: Main<WlSurface>,
    frame: Option<Frame>,
}

impl Connection {
    pub fn create_window(&self, builder: mwin::WindowBuilder) -> Result<mwin::Window, OSError> {
        let wl_surface = self.compositor.create_surface();
        let surface_id = wl_surface.as_ref().id();
        let xdg_surface = self.xdg_wm_base.get_xdg_surface(&wl_surface);
        let xdg_toplevel = xdg_surface.get_toplevel();

        xdg_toplevel.set_title(builder.title.clone());

        let top_level_ev_sender = self.events_sender.clone();
        xdg_toplevel.quick_assign(move |_, event, _| {
            use wayland_protocols::xdg_shell::client::xdg_toplevel::Event as WlEvent;
            let window = WindowId::from_wayland(surface_id);
            match event {
                WlEvent::Configure { .. } => {}
                WlEvent::Close => {
                    top_level_ev_sender
                        .send(Event::WindowEvent {
                            window,
                            event: WindowEvent::CloseRequested,
                        })
                        .unwrap();
                }
                _ => {}
            }
        });

        xdg_surface.quick_assign(|xdg_surface, event, _| {
            use wayland_protocols::xdg_shell::client::xdg_surface::Event;
            match event {
                Event::Configure { serial } => {
                    xdg_surface.ack_configure(serial);
                }
                _ => {}
            }
        });

        wl_surface.commit();

        if builder.decorations {
            return self.build_framed_window(builder, xdg_toplevel, wl_surface);
        }

        let buf_x: i32 = builder.width as i32;
        let buf_y: i32 = builder.height as i32;

        let (frame_buffer_ptr, frame_buffer_len) = self.setup_surface(&wl_surface, buf_x, buf_y);

        let frame_buffer_ptr = AtomicPtr::new(frame_buffer_ptr);
        let shared_data = surface::SharedData {
            buffer_len: frame_buffer_len,
            width: buf_x as u32,
            height: buf_y as u32,
        };

        let surface = Surface::new(
            surface::Format::Argb8888,
            Arc::new((frame_buffer_ptr, Atomic::new(shared_data))),
        );

        let window = Arc::new(RwLock::new(WindowPlatformData::Wayland(Window {
            xdg_toplevel,
            surface: surface.clone(),
            wl_surface,
            buf_x,
            buf_y,
            frame: None,
        })));
        self.windows
            .write()
            .insert(WindowId::from_wayland(surface_id), window.clone());
        Ok(mwin::Window {
            id: WindowId::from_wayland(surface_id),
            surface,
            logical_size: Arc::new(Atomic::new(LogicalSize {
                w: builder.width,
                h: builder.height,
            })),
            dpi: Arc::new(Atomic::new(1.0)),
            platform_data: window,
        })
    }

    pub fn redraw_window(&self, window: &Window) {
        window.wl_surface.damage(0, 0, window.buf_x, window.buf_y);
        window.wl_surface.commit();
        if let Some(ref frame) = window.frame {
            frame
                .wl_surface
                .damage(0, 0, frame.frame_width, frame.header_bar_height);
            frame.wl_surface.commit();
        }
    }

    pub fn destroy_window(&self, window: &mut Window) -> Result<(), OSError> {
        window.xdg_toplevel.destroy();
        unsafe {
            munmap(
                window.surface.shared().0.load(Ordering::SeqCst) as *mut _,
                window.surface.shared().1.load(Ordering::SeqCst).buffer_len,
            );
        }
        window.wl_surface.destroy();
        if let Some(ref frame) = window.frame {
            unsafe {
                munmap(
                    frame.surface.shared().0.load(Ordering::SeqCst) as *mut _,
                    frame.surface.shared().1.load(Ordering::SeqCst).buffer_len,
                );
            }
            frame.wl_surface.destroy();
        }
        window.xdg_toplevel.destroy();
        self.windows
            .write()
            .remove(&WindowId::from_wayland(window.wl_surface.as_ref().id()));
        Ok(())
    }

    fn setup_surface(
        &self,
        buffer_surface: &Main<WlSurface>,
        buf_width: i32,
        buf_height: i32,
    ) -> (*mut u8, usize) {
        let buf_len = (buf_width * buf_height) * 4;
        let tmp = tempfile::tempfile().expect("Unable to create a tempfile.");
        tmp.set_len(buf_len as u64).unwrap();

        let pool = self.shm.create_pool(
            tmp.as_raw_fd(), // RawFd to the tempfile serving as shared memory
            buf_len,         // size in bytes of the shared memory (4 bytes per pixel)
        );
        let buffer = pool.create_buffer(
            0,                      // Start of the buffer in the pool
            buf_width,              // width of the buffer in pixels
            buf_height,             // height of the buffer in pixels
            (buf_width * 4) as i32, // number of bytes between the beginning of two consecutive lines
            Format::Argb8888,       // chosen encoding for the data
        );

        self.event_queue
            .lock()
            .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
            .unwrap();

        buffer_surface.attach(Some(&buffer), 0, 0);
        buffer_surface.commit();

        self.event_queue
            .lock()
            .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
            .unwrap();

        let buf_len = buf_len as usize;

        let in_memory_addr = unsafe {
            mmap(
                null_mut(),
                buf_len,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                tmp.as_raw_fd(),
                0,
            )
        };
        assert_ne!(in_memory_addr, MAP_FAILED);

        (in_memory_addr as *mut _, buf_len)
    }
}
