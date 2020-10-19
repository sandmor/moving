use super::super::{Connection, WindowId, WindowPlatformData};
use super::Window;
use crate::{
    dpi::LogicalSize,
    error::OSError,
    surface::{self, Surface},
    window as mwin,
};
use atomic::Atomic;
use parking_lot::RwLock;
use std::sync::{atomic::AtomicPtr, Arc};
use wayland_client::{protocol::wl_surface::WlSurface, Main};
use wayland_protocols::xdg_shell::client::xdg_toplevel::XdgToplevel;

#[derive(Debug)]
pub struct Frame {
    pub wl_surface: Main<WlSurface>,
    pub surface: Surface,
    pub frame_width: i32,
    pub header_bar_height: i32,
}

impl Connection {
    pub fn build_framed_window(
        &self,
        builder: mwin::WindowBuilder,
        xdg_toplevel: Main<XdgToplevel>,
        frame_wl_surface: Main<WlSurface>,
    ) -> Result<mwin::Window, OSError> {
        let header_bar_height = 20;

        let frame_width = builder.width as i32;
        let frame_height = header_bar_height + builder.height as i32;
        let (frame_buffer_ptr, frame_buffer_len) =
            self.setup_surface(&frame_wl_surface, frame_width, frame_height);

        let frame_buffer_ptr = AtomicPtr::new(frame_buffer_ptr);
        let frame_shared_data = surface::SharedData {
            buffer_len: frame_buffer_len,
            width: frame_width as u32,
            height: frame_height as u32,
        };
        let frame_surface = Surface::new(
            surface::Format::Argb8888,
            Arc::new((frame_buffer_ptr, Atomic::new(frame_shared_data))),
        );

        let buffer_surface = self.compositor.create_surface();
        let buffer_surface_id = buffer_surface.as_ref().id();
        let buffer_subsurface = self
            .subcompositor
            .get_subsurface(&buffer_surface, &frame_wl_surface);
        buffer_subsurface.set_position(0, header_bar_height);
        let (frame_buffer_ptr, frame_buffer_len) =
            self.setup_surface(&buffer_surface, builder.width as i32, builder.height as i32);

        for y in 0..header_bar_height as u32 {
            for x in 0..frame_width as u32 {
                frame_surface.put_u32_pixel(x, y, 0xffdadada);
            }
        }

        let frame = Frame {
            wl_surface: frame_wl_surface,
            surface: frame_surface,
            frame_width,
            header_bar_height,
        };

        let frame_buffer_ptr = AtomicPtr::new(frame_buffer_ptr);
        let shared_data = surface::SharedData {
            buffer_len: frame_buffer_len,
            width: builder.width as u32,
            height: builder.height as u32,
        };

        let surface = Surface::new(
            surface::Format::Argb8888,
            Arc::new((frame_buffer_ptr, Atomic::new(shared_data))),
        );

        let window = Arc::new(RwLock::new(WindowPlatformData::Wayland(Window {
            xdg_toplevel,
            surface: surface.clone(),
            wl_surface: buffer_surface,
            buf_x: builder.width as i32,
            buf_y: builder.height as i32,
            frame: Some(frame),
        })));
        self.windows
            .write()
            .insert(WindowId::from_wayland(buffer_surface_id), window.clone());
        Ok(mwin::Window {
            id: WindowId::from_wayland(buffer_surface_id),
            surface,
            logical_size: Arc::new(Atomic::new(LogicalSize {
                w: builder.width,
                h: builder.height,
            })),
            dpi: Arc::new(Atomic::new(1.0)),
            platform_data: window,
        })
    }
}
