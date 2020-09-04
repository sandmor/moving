use crate::platform::WindowId;
use crate::Rect;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    WindowEvent {
        window: WindowId,
        event: WindowEvent
    },
    MainEventsCleared,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    CloseRequested,
    Damaged(Rect, usize),
    Destroy,
}