pub use winit::{dpi, error, event, event_loop, monitor};

mod platform;

pub mod window;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
