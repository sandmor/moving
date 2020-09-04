use moving::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use std::time::Instant;

fn main() {
    let event_loop = EventLoop::new();
    let mut window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut start = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                *control_flow = ControlFlow::Exit
            }
            Event::MainEventsCleared => {
                let w = window.width() as usize;
                let h = window.height() as usize;
                let frame_buffer = window.frame_buffer();
                let mut x = 0.0;
                let mut y = 0.0;
                let fw = w as f64;
                let fh = h as f64;
                let cx = fw / 2.0;
                let cy = fh / 2.0;
                let t = Instant::now().duration_since(start).as_millis() as f64;
                for i in 0..w * h {
                    x += 1.0;
                    if x >= fw {
                        x = 0.0;
                        y += 1.0;
                    }
                    let rx = x - cx;
                    let ry = y - cy;
                    frame_buffer[i * 4] =
                        ((((f64::sqrt(rx * rx + ry * ry) + (f64::atan2(rx, ry) * 40.0 + t)) * 10.0)
                            % 512.0)-256.0).abs() as u8;
                }
                window.redraw();
            }
            _ => (),
        }
    });
}
