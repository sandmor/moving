use moving::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use std::time::Instant;

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let start = Instant::now();
    event_loop.run(move |event, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                *control_flow = ControlFlow::Exit
            }
            Event::MainEventsCleared => {
                let size = window.size();
                let w = size.width;
                let h = size.height;
                let frame_buffer = window.frame_buffer();
                let mut x = 0.0;
                let mut y = 0.0;
                let cx = w / 2.0;
                let cy = h / 2.0;
                let t = Instant::now().duration_since(start).as_millis() as f64;
                for i in 0..frame_buffer.len() / 4 {
                    x += 1.0;
                    if x >= w {
                        x = 0.0;
                        y += 1.0;
                    }
                    let rx = x - cx;
                    let ry = y - cy;
                    frame_buffer[i * 4] = ((((f64::sqrt(rx * rx + ry * ry)
                        + (f64::atan2(rx, ry) * 40.0 + t))
                        * 10.0)
                        % 512.0)
                        - 256.0)
                        .abs() as u8;
                    frame_buffer[i * 4 + 3] = 128;
                }
                window.redraw();
            }
            _ => (),
        }
    });
}
