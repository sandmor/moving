use parking_lot::RwLock;
use crate::{error::OSError, event_loop::EventLoop};
use crate::{Size, platform::WindowId};
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct WindowInner {
    pub size: Size,
    pub frame_buffer_ptr: *mut u8,
    pub frame_buffer_len: usize,
}

#[derive(Debug)]
pub struct Window {
    pub(crate) id: WindowId,
    pub(crate) inner: Arc<RwLock<WindowInner>>,
    pub(crate) platform: Arc<RwLock<crate::platform::WindowPlatform>>
}

impl Window {
    pub fn size(&self) -> Size {
        self.inner.read().size
    }

    pub fn width(&self) -> f64 {
        self.inner.read().size.width
    }

    pub fn height(&self) -> f64 {
        self.inner.read().size.height
    }

    pub fn frame_buffer(&self) -> &mut [u8] {
        let inner = self.inner.read();
        unsafe { std::slice::from_raw_parts_mut(inner.frame_buffer_ptr, inner.frame_buffer_len) }
    }

    pub fn redraw(&self) {
        crate::platform::redraw_window(&self);
    }
}

#[derive(Debug)]
pub struct WindowBuilder {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl WindowBuilder {
    pub fn new() -> WindowBuilder {
        WindowBuilder { width: 800.0, height: 600.0 }
    }

    pub fn build(self, el: &EventLoop) -> Result<Window, OSError> {
        el.create_window(self)
    }
}