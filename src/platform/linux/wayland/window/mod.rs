mod frame;
use frame::*;

use super::Connection;
use crate::{
    error::OSError,
    event::*,
    platform::{WindowId, WindowPlatformData},
    window as mwin, Size,
};
use libc::{mmap, munmap, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::RwLock;
use std::{
    os::unix::io::AsRawFd,
    ptr::{null_mut, NonNull},
    sync::Arc,
};
use wayland_client::{
    protocol::{wl_shm::Format, wl_shm_pool::WlShmPool, wl_surface::WlSurface},
    Main,
};
use wayland_protocols::xdg_shell::client::xdg_toplevel::XdgToplevel;

#[derive(Debug)]
pub struct Window {
    xdg_toplevel: Main<XdgToplevel>,
    surface: Main<WlSurface>,
    buf_x: i32,
    buf_y: i32,
    pixels_box: Arc<RwLock<mwin::PixelsBox>>,
    frame: Option<Frame>,
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
        let surface = self.compositor.create_surface();
        let surface_id = surface.as_ref().id();
        let xdg_surface = self.xdg_wm_base.get_xdg_surface(&surface);
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

        surface.commit();

        if builder.decorations {
            return self.build_framed_window(builder, xdg_toplevel, xdg_surface, surface);
        }

        let buf_x: i32 = builder.width as i32;
        let buf_y: i32 = builder.height as i32;

        let (frame_buffer_ptr, frame_buffer_len) = self.setup_surface(&surface, buf_x, buf_y);

        let pixels_box = Arc::new(RwLock::new(mwin::PixelsBox::from_raw(
            Size::new(builder.width, builder.height),
            frame_buffer_ptr,
            frame_buffer_len,
        )));

        let window = Arc::new(RwLock::new(WindowPlatformData::Wayland(Window {
            xdg_toplevel,
            surface,
            buf_x,
            buf_y,
            pixels_box: pixels_box.clone(),
            frame: None,
        })));
        self.windows.write().insert(WindowId::from_wayland(surface_id), window.clone());
        Ok((surface_id, window, pixels_box))
    }

    pub fn redraw_window(&self, window: &Window) {
        window.surface.damage(0, 0, window.buf_x, window.buf_y);
        window.surface.commit();
        if let Some(ref frame) = window.frame {
            frame
                .surface
                .damage(0, 0, frame.frame_width, frame.header_bar_height);
            frame.surface.commit();
        }
    }

    pub fn destroy_window(&self, window: &mut Window) -> Result<(), OSError> {
        window.xdg_toplevel.destroy();
        unsafe {
            munmap(
                window.pixels_box.read().frame_buffer_ptr().as_ptr() as *mut _,
                window.pixels_box.read().frame_buffer_len(),
            );
        }
        window.surface.destroy();
        if let Some(ref frame) = window.frame {
            unsafe {
                munmap(
                    frame.pixels_box.frame_buffer_ptr().as_ptr() as *mut _,
                    frame.pixels_box.frame_buffer_len(),
                );
            }
            frame.surface.destroy();
        }
        window.xdg_toplevel.destroy();
        self.windows.write().remove(&WindowId::from_wayland(window.surface.as_ref().id()));
        Ok(())
    }

    fn setup_surface(
        &self,
        buffer_surface: &Main<WlSurface>,
        buf_width: i32,
        buf_height: i32,
    ) -> (NonNull<u8>, usize) {
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

        let buf_ptr = NonNull::new(in_memory_addr as *mut u8).unwrap();
        (buf_ptr, buf_len)
    }
}
