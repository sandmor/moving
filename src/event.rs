use crate::platform::WindowId;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum ButtonState {
    Released,
    Pressed
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
    Resize {
        width: f64,
        height: f64,
    },
    MouseButton {
        x: f64,
        y: f64,
        state: ButtonState,
        button: MouseButton
    },
    MouseMove {
        x: f64,
        y: f64,
    },
    MouseEnter {
        x: f64,
        y: f64,
    },
    MouseLeave {
        x: f64,
        y: f64,
    },
}
