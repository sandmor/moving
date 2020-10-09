use moving::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use std::time::Instant;

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("H-Tree")
        .build(&event_loop)
        .unwrap();
    let start = Instant::now();
    event_loop.run(move |event, control_flow| {
        *control_flow = ControlFlow::Wait;

        let event = match event {
            Event::WindowEvent { event, .. } => event,
            _ => {
                return;
            }
        };
        match event {
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
            WindowEvent::MouseMove { x, y } => {}
            _ => {}
        }
    });
}
