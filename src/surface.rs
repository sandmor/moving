use atomic::Atomic;
use std::{sync::{
    atomic::{AtomicPtr, Ordering},
    Arc,
}, slice};

pub(crate) type Shared = Arc<(AtomicPtr<u8>, Atomic<SharedData>)>;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum Format {
    Argb8888,
    Xrgb8888,
}

// NOTE: Default must be supported by all platforms and must not fail
impl Default for Format {
    fn default() -> Self {
        Self::Argb8888
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct SharedData {
    pub buffer_len: usize,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Surface {
    format: Format,
    shared: Arc<(AtomicPtr<u8>, Atomic<SharedData>)>,
}

impl Surface {
    pub(crate) fn new(format: Format, shared: Shared) -> Self {
        Self { format, shared }
    }

    pub(crate) fn shared(&self) -> Arc<(AtomicPtr<u8>, Atomic<SharedData>)> {
        self.shared.clone()
    }

    pub fn width(&self) -> u32 {
        self.shared.1.load(Ordering::SeqCst).width
    }

    pub fn height(&self) -> u32 {
        self.shared.1.load(Ordering::SeqCst).height
    }

    pub fn size(&self) -> (u32, u32) {
        let shared = self.shared.1.load(Ordering::SeqCst);
        (shared.width, shared.height)
    }

    /// Note that although it takes an immutable reference to self, it sets a pixel in the buffer
    /// this is made for simplify parallerization processes
    pub fn put_u32_pixel(&self, x: u32, y: u32, pixel: u32) {
        let size_data = self.shared.1.load(Ordering::SeqCst);
        let offset = ((y * size_data.width) + x) as usize;
        if (offset + 1) * 4 > size_data.buffer_len {
            return;
        }
        unsafe {
            *(self.shared.0.load(Ordering::SeqCst) as *mut u32).offset(offset as isize) = pixel;
        }
    }

    pub fn data_mut(&self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(self.shared.0.load(Ordering::SeqCst), self.shared.1.load(Ordering::SeqCst).buffer_len)
        }
    }
}
