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
    pool: Main<WlShmPool>,
    surface: Main<WlSurface>,
    buf_x: i32,
    buf_y: i32,
    on_slab_offset: usize,
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
        let buf_x: i32 = builder.width as i32;
        let buf_y: i32 = builder.height as i32;

        let surface = self.compositor.create_surface();
        let xdg_surface = self.xdg_wm_base.get_xdg_surface(&surface);
        let xdg_toplevel = xdg_surface.get_toplevel();

        xdg_toplevel.set_title(builder.title.clone());

        xdg_surface.quick_assign(|xdg_surface, event, _| {
            use wayland_protocols::xdg_shell::client::xdg_surface::Event;
            match event {
                Event::Configure { serial } => {
                    xdg_surface.ack_configure(serial);
                }
                _ => {}
            }
        });
        let top_level_ev_sender = self.events_sender.clone();
        xdg_toplevel.quick_assign(move |_, event, _| {
            use wayland_protocols::xdg_shell::client::xdg_toplevel::Event as WlEvent;
            let window = WindowId(0);
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
        let buf_len = (buf_x * buf_y) * 4;
        surface.commit();
        let tmp = tempfile::tempfile().expect("Unable to create a tempfile.");
        tmp.set_len(buf_len as u64).unwrap();

        let pool = self.shm.create_pool(
            tmp.as_raw_fd(), // RawFd to the tempfile serving as shared memory
            buf_len,         // size in bytes of the shared memory (4 bytes per pixel)
        );
        let buffer = pool.create_buffer(
            0,                  // Start of the buffer in the pool
            buf_x,              // width of the buffer in pixels
            buf_y,              // height of the buffer in pixels
            (buf_x * 4) as i32, // number of bytes between the beginning of two consecutive lines
            Format::Argb8888,   // chosen encoding for the data
        );

        self.event_queue
            .lock()
            .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
            .unwrap();

        surface.attach(Some(&buffer), 0, 0);
        surface.commit();

        self.event_queue
            .lock()
            .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
            .unwrap();

        let frame_buffer_len = buf_len as usize;

        let in_memory_addr = unsafe {
            mmap(
                null_mut(),
                frame_buffer_len,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                tmp.as_raw_fd(),
                0,
            )
        };
        assert_ne!(in_memory_addr, MAP_FAILED);

        let frame_buffer_ptr = NonNull::new(in_memory_addr as *mut u8).unwrap();

        let pixels_box = Arc::new(RwLock::new(mwin::PixelsBox::from_raw(
            Size::new(builder.width, builder.height),
            frame_buffer_ptr,
            frame_buffer_len,
        )));

        let window = Arc::new(RwLock::new(WindowPlatformData::Wayland(Window {
            xdg_toplevel,
            pool,
            surface,
            buf_x,
            buf_y,
            on_slab_offset: 0,
            pixels_box: pixels_box.clone(),
        })));
        let id = self.windows.write().insert(window.clone());
        window.write().wayland_mut().on_slab_offset = id;
        Ok((id as u32, window, pixels_box))
    }

    pub fn redraw_window(&self, window: &Window) {
        window.surface.damage(0, 0, window.buf_x, window.buf_y);
        window.surface.commit();
    }

    pub fn destroy_window(&self, window: &mut Window) -> Result<(), OSError> {
        window.xdg_toplevel.destroy();
        unsafe {
            munmap(
                window.pixels_box.read().frame_buffer_ptr().as_ptr() as *mut _,
                window.pixels_box.read().frame_buffer_len(),
            );
        }
        self.windows.write().remove(window.on_slab_offset);
        Ok(())
    }
}
