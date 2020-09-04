use crate::error::OsError;
use crate::event_loop::EventLoopWindowTarget;
use crate::platform::*;
use raw_window_handle::HasRawWindowHandle;
use winit::window::Window as WWindow;
use winit::window::WindowBuilder as WWindowBuilder;

pub use winit::window::{BadIcon, CursorIcon, Fullscreen, Icon, Theme, WindowAttributes, WindowId};

#[derive(Debug)]
pub struct Window {
    inner: WWindow,
    handle: Handle,
}

impl Window {
    pub fn new<T: 'static>(event_loop: &EventLoopWindowTarget<T>) -> Result<Window, OsError> {
        WindowBuilder::new().build(event_loop)
    }

    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }

    pub fn frame_buffer(&mut self) -> &mut [u8] {
        self.handle.frame_buffer()
    }

    pub fn redraw(&mut self) {
        self.handle.redraw();
    }

    pub fn width(&self) -> u32 {
        self.handle.width()
    }

    pub fn height(&self) -> u32 {
        self.handle.height()
    }
}

#[derive(Debug)]
pub struct WindowBuilder {
    inner: WWindowBuilder,
}

impl WindowBuilder {
    pub fn new() -> WindowBuilder {
        WindowBuilder {
            inner: WWindowBuilder::new(),
        }
    }

    pub fn build<T: 'static>(
        self,
        window_target: &EventLoopWindowTarget<T>,
    ) -> Result<Window, OsError> {
        let winit_win = self.inner.build(window_target)?;
        let handle = handle(winit_win.raw_window_handle());
        Ok(Window {
            inner: winit_win,
            handle,
        })
    }
}
