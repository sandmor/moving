use super::*;
use crate::{window as mwin, Size};
use orbclient::{renderer::Renderer, Window};
use parking_lot::RwLock;
use std::{fmt, ptr::NonNull, sync::Arc};

pub struct WindowPlatformData {
    buffer: Vec<u8>,
    orb_window: Arc<RwLock<Window>>,
    pixels_box: Arc<RwLock<mwin::PixelsBox>>,
}

impl fmt::Debug for WindowPlatformData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowPlatformData").finish()
    }
}

impl Connection {
    pub fn create_window(&self, builder: mwin::WindowBuilder) -> Result<mwin::Window, OSError> {
        let orb_window = Window::new(0, 0, builder.width as u32, builder.height as u32, "")
            .ok_or_else(|| OSError::Unknown)?;

        let frame_buffer_len = orb_window.data().len();
        let mut buffer = Vec::with_capacity(frame_buffer_len);

        let frame_buffer_ptr: NonNull<u8> =
            NonNull::new(buffer.as_mut_slice().as_mut_ptr()).unwrap();

        // todo use correct window id
        let id = WindowId::from_orbclient(0);

        let pixels_box = Arc::new(RwLock::new(mwin::PixelsBox::from_raw(
            Size::new(builder.width, builder.height),
            frame_buffer_ptr,
            frame_buffer_len,
        )));

        let platform_data = Arc::new(RwLock::new(WindowPlatformData {
            buffer,
            orb_window: Arc::new(RwLock::new(orb_window)),
            pixels_box: pixels_box.clone(),
        }));
        Ok(mwin::Window {
            id,
            pixels_box,
            platform_data,
        })
    }

    pub fn destroy_window(&self, window: &mut WindowPlatformData) -> Result<(), OSError> {
        todo!()
    }

    pub fn redraw_window(&self, window: &mwin::Window) {
        let color_data: Vec<orbclient::Color> = window
            .platform_data
            .read()
            .buffer
            .iter()
            .map(|v| orbclient::Color { data: *v as u32 })
            .collect();

        window
            .platform_data
            .read()
            .orb_window
            .write()
            .data_mut()
            .clone_from_slice(color_data.as_slice());
    }
}
