use crate::platform::WindowId;

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
}
