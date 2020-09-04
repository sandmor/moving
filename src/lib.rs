mod platform;

pub mod window;
pub mod event;
pub mod event_loop;
pub mod error;

pub type Rect = euclid::Rect<f64, ()>;
pub type Point = euclid::Point2D<f64, ()>;
pub type Size = euclid::Size2D<f64, ()>;

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    euclid::rect(x, y, w, h)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
