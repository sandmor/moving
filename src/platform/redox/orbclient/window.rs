use parking_lot::RwLock;
use std::sync::Arc;

use orbclient::{renderer::Renderer, Window as OrbWindow};

use crate::{
    error::OSError,
    platform::WindowId,
    window::{Window as MWindow, WindowBuilder, WindowInner},
    Size,
};
pub struct WindowPlatform {
    buffer: Vec<u8>,
    orb_window: Arc<RwLock<OrbWindow>>,
    inner: Arc<RwLock<WindowInner>>,
}

impl std::fmt::Debug for WindowPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowPlatform").finish()
    }
}

pub fn create_window(builder: WindowBuilder) -> Result<MWindow, OSError> {
    // todo use correct error

    let orb_window = OrbWindow::new(0, 0, builder.width as u32, builder.height as u32, "")
        .ok_or_else(|| OSError::Unknown)?;

    let frame_buffer_len = orb_window.data().len();
    let mut buffer = Vec::with_capacity(frame_buffer_len);

    let frame_buffer_ptr: *mut u8 = buffer.as_mut_slice().as_mut_ptr();

    // todo use correct window id
    let id = WindowId::from_orbclient(0);

    let inner = Arc::new(RwLock::new(WindowInner {
        size: Size::new(builder.width, builder.height),
        frame_buffer_ptr,
        frame_buffer_len,
    }));

    let window = WindowPlatform {
        buffer,
        orb_window: Arc::new(RwLock::new(orb_window)),
        inner: inner.clone(),
    };

    Ok(MWindow {
        id,
        inner,
        platform: Arc::new(RwLock::new(crate::platform::WindowPlatform::OrbClient(
            window,
        ))),
    })
}

pub fn redraw_window(id: WindowId, platform: &WindowPlatform) {
    let color_data: Vec<orbclient::Color> = platform
        .buffer
        .iter()
        .map(|v| orbclient::Color { data: *v as u32 })
        .collect();

    platform
        .orb_window
        .write()
        .data_mut()
        .clone_from_slice(color_data.as_slice());
}
