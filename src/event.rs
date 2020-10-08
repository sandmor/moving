use bitflags::bitflags;
use crate::platform::WindowId;

bitflags! {
    pub struct MouseButtons: u8 {
        const LEFT_BUTTON = 0b00000001;
        const RIGHT_BUTTON = 0b00000010;
        const MIDDLE_BUTTON = 0b00000100;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    WindowEvent {
        window: WindowId,
        event: WindowEvent,
    },
    MainEventsCleared,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    CloseRequested,
    Dirted,
    Destroy,
    Resize { width: f64, height: f64 },
    MouseUp { x: f64, y: f64, buttons: MouseButtons },
    MouseDown { x: f64, y: f64, buttons: MouseButtons },
    MouseMove { x: f64, y: f64 },
}
