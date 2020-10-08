use crate::{
    error::OSError,
    event_loop::EventLoop,
    platform::{WindowId, WindowPlatformData},
    Size, CONNECTION,
};
use parking_lot::RwLock;
use std::{ptr::NonNull, sync::Arc};

#[derive(Debug)]
pub(crate) struct WindowInner {
    pub size: Size,
    pub frame_buffer_ptr: *mut u8,
    pub frame_buffer_len: usize,
}

#[derive(Debug)]
pub struct PixelsBox {
    size: Size,
    frame_buffer_ptr: NonNull<u8>,
    frame_buffer_len: usize,
}

impl PixelsBox {
    pub(crate) fn from_raw(
        size: Size,
        frame_buffer_ptr: NonNull<u8>,
        frame_buffer_len: usize,
    ) -> Self {
        Self {
            size,
            frame_buffer_ptr,
            frame_buffer_len,
        }
    }
}

unsafe impl Sync for PixelsBox {}
unsafe impl Send for PixelsBox {}

/// Be careful the windows support transparency and are fully transparent at the start
pub struct Window {
    pub(crate) id: WindowId,
    pub(crate) pixels_box: Arc<RwLock<PixelsBox>>,
    // This is used to store platform specifid information
    pub(crate) platform_data: Arc<RwLock<WindowPlatformData>>,
}

impl Window {
    pub fn size(&self) -> Size {
        self.pixels_box.read().size
    }

    pub fn width(&self) -> f64 {
        self.pixels_box.read().size.width
    }

    pub fn height(&self) -> f64 {
        self.pixels_box.read().size.height
    }

    pub fn frame_buffer(&self) -> &mut [u8] {
        let pixels_box = self.pixels_box.read();
        unsafe {
            std::slice::from_raw_parts_mut(
                pixels_box.frame_buffer_ptr.as_ptr(),
                pixels_box.frame_buffer_len,
            )
        }
    }

    pub fn redraw(&self) {
        CONNECTION.redraw_window(&self);
    }
}

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub struct WindowBuilder {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl WindowBuilder {
    pub fn new() -> WindowBuilder {
        WindowBuilder {
            width: 800.0,
            height: 600.0,
        }
    }

    pub fn with_width(&mut self, width: f64) -> Self {
        self.width = width;
        *self
    }

    pub fn with_height(&mut self, height: f64) -> Self {
        self.height = height;
        *self
    }

    pub fn with_size(&mut self, width: f64, height: f64) -> Self {
        self.width = width;
        self.height = height;
        *self
    }

    pub fn build(self, el: &EventLoop) -> Result<Window, OSError> {
        el.create_window(self)
    }
}
