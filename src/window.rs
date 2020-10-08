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

    pub fn pixels_mut(&self) -> impl Iterator<Item=(usize, usize, &mut u32)> {
        todo!()
    }

    /// Note that although it takes an immutable reference to self, it sets a pixel in the buffer
    /// this is made for simplify parallerization processes
    pub fn put_pixel(&self, x: usize, y: usize, color: u32) {
        let width = self.size.width as usize;
        let offset = (y * width) + x;
        if offset >= self.frame_buffer_len {
            return;
        }
        unsafe {
            *(self.frame_buffer_ptr.as_ptr() as *mut u32).offset(offset as isize) = color;
        }
    }
}

unsafe impl Sync for PixelsBox {}
unsafe impl Send for PixelsBox {}

/// Be careful the windows support transparency and are fully transparent at the start
pub struct Window {
    pub(crate) id: WindowId,
    pub(crate) pixels_box: Arc<RwLock<PixelsBox>>,
    // This is used to store platform-specific information
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

    pub fn pixels_box(&self) -> Arc<RwLock<PixelsBox>> {
        self.pixels_box.clone()
    }

    pub fn redraw(&self) {
        CONNECTION.redraw_window(&self);
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub struct WindowBuilder {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) title: String,
    pub(crate) decorations: bool,
}

impl WindowBuilder {
    pub fn new() -> WindowBuilder {
        WindowBuilder {
            width: 800.0,
            height: 600.0,
            title: String::new(),
            decorations: true
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }

    pub fn with_width(mut self, width: f64) -> Self {
        self.width = width;
        self
    }

    pub fn with_height(mut self, height: f64) -> Self {
        self.height = height;
        self
    }

    pub fn with_size(mut self, width: f64, height: f64) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn build(self, el: &EventLoop) -> Result<Window, OSError> {
        el.create_window(self)
    }
}
