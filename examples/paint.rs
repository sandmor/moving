use moving::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use std::time::Instant;

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Paint example")
        .build(&event_loop)
        .unwrap();
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
                let surface = window.surface();
                let cx = (surface.width() as f64) / 2.0;
                let cy = (surface.height() as f64) / 2.0;
                let t = Instant::now().duration_since(start).as_millis() as f64;
                for y in 0..surface.height() {
                    for x in 0..surface.width() {
                        let rx = (x as f64) - cx;
                        let ry = (y as f64) - cy;
                        let blue = ((((f64::sqrt(rx * rx + ry * ry)
                            + (f64::atan2(rx, ry) * 40.0 + t))
                            * 10.0)
                            % 512.0)
                            - 256.0)
                            .abs() as u8;
                        surface.put_u32_pixel(x as u32, y as u32, (128 << 24) | (blue as u32));
                    }
                }
                window.redraw();
            }
            _ => (),
        }
    });
}
