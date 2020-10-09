use super::super::{Connection, WindowPlatformData};
use super::Window;
use crate::{error::OSError, window as mwin, Size};
use libc::{mmap, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::RwLock;
use std::{
    os::unix::io::AsRawFd,
    ptr::{null_mut, NonNull},
    sync::Arc,
};
use wayland_client::{
    protocol::{wl_shm::Format, wl_surface::WlSurface},
    Main,
};
use wayland_protocols::xdg_shell::client::{xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel};

#[derive(Debug)]
pub struct Frame {
    pub surface: Main<WlSurface>,
    pub pixels_box: mwin::PixelsBox,
    pub frame_width: i32,
    pub header_bar_height: i32,
}

impl Connection {
    pub fn build_framed_window(
        &self,
        builder: mwin::WindowBuilder,
        xdg_toplevel: Main<XdgToplevel>,
        xdg_surface: Main<XdgSurface>,
        frame_surface: Main<WlSurface>,
    ) -> Result<
        (
            u32,
            Arc<RwLock<WindowPlatformData>>,
            Arc<RwLock<mwin::PixelsBox>>,
        ),
        OSError,
    > {
        let header_bar_height = 20;

        let frame_width = builder.width as i32;
        let frame_height = header_bar_height + builder.height as i32;
        let (frame_buffer_ptr, frame_buffer_len) =
            self.setup_surface(&frame_surface, frame_width, frame_height);
        let pixels_box = mwin::PixelsBox::from_raw(
            Size::new(frame_width as f64, frame_height as f64),
            frame_buffer_ptr,
            frame_buffer_len,
        );

        let buffer_surface = self.compositor.create_surface();
        let buffer_subsurface = self
            .subcompositor
            .get_subsurface(&buffer_surface, &frame_surface);
        buffer_subsurface.set_position(0, header_bar_height);
        let (frame_buffer_ptr, frame_buffer_len) =
            self.setup_surface(&buffer_surface, builder.width as i32, builder.height as i32);

        for y in 0..header_bar_height as usize {
            for x in 0..frame_width as usize {
                pixels_box.put_pixel(x, y, 0xffdadada);
            }
        }

        let frame = Frame {
            surface: frame_surface,
            pixels_box,
            frame_width,
            header_bar_height,
        };

        let pixels_box = Arc::new(RwLock::new(mwin::PixelsBox::from_raw(
            Size::new(builder.width, builder.height),
            frame_buffer_ptr,
            frame_buffer_len,
        )));

        let window = Arc::new(RwLock::new(WindowPlatformData::Wayland(Window {
            xdg_toplevel,
            surface: buffer_surface,
            buf_x: builder.width as i32,
            buf_y: builder.height as i32,
            on_slab_offset: 0,
            pixels_box: pixels_box.clone(),
            frame: Some(frame),
        })));
        let id = self.windows.write().insert(window.clone());
        window.write().wayland_mut().on_slab_offset = id;
        Ok((id as u32, window, pixels_box))
    }
}
