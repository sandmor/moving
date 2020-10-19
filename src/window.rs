use crate::{
    dpi,
    error::OSError,
    event_loop::EventLoop,
    platform::{WindowId, WindowPlatformData},
    surface, CONNECTION,
};
use atomic::Atomic;
use parking_lot::RwLock;
use std::sync::{atomic::Ordering, Arc};

/// Be careful the windows support transparency and are fully transparent at the start
#[derive(Debug)]
pub struct Window {
    pub(crate) id: WindowId,
    pub(crate) surface: surface::Surface,
    pub(crate) logical_size: Arc<Atomic<dpi::LogicalSize>>,
    pub(crate) dpi: Arc<Atomic<dpi::Dpi>>,
    // This is used to store platform-specific information
    pub(crate) platform_data: Arc<RwLock<WindowPlatformData>>,
}

impl Window {
    pub fn dpi(&self) -> dpi::Dpi {
        self.dpi.load(Ordering::SeqCst)
    }

    pub fn logical_size(&self) -> dpi::LogicalSize {
        self.logical_size.load(Ordering::SeqCst)
    }

    pub fn physical_size(&self) -> dpi::PhysicalSize {
        self.logical_size().to_physical(self.dpi())
    }

    pub fn surface(&self) -> surface::Surface {
        self.surface.clone()
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
    pub(crate) surface_format: surface::Format,
}

impl WindowBuilder {
    pub fn new() -> WindowBuilder {
        WindowBuilder {
            width: 800.0,
            height: 600.0,
            title: String::new(),
            decorations: true,
            surface_format: surface::Format::default(),
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

    pub fn with_surface_format(mut self, format: surface::Format) -> Self {
        self.surface_format = format;
        self
    }

    pub fn build(self, el: &EventLoop) -> Result<Window, OSError> {
        el.create_window(self)
    }
}
