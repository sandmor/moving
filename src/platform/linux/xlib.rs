use lazy_static::lazy_static;
use raw_window_handle::unix::XlibHandle as RawXlibHandle;
use std::mem::MaybeUninit;
use std::os::raw::*;
use x11_dl::xlib::{self, Xlib};

lazy_static! {
    static ref XLIB: Xlib = Xlib::open().expect("Failed to open Xlib");
}

#[derive(Debug)]
pub struct XlibHandle {
    window: c_ulong,
    display: *mut xlib::Display,
    window_attrs: xlib::XWindowAttributes,
    frame_buffer: Vec<u8>,
    pixmap: c_ulong,
    image: *mut xlib::XImage,
}

impl XlibHandle {
    pub fn new(raw: RawXlibHandle) -> XlibHandle {
        let window = raw.window;
        let display = raw.display as *mut xlib::Display;
        let window_attrs;
        let mut frame_buffer;
        let pixmap;
        let image;
        unsafe {
            let mut attrs = MaybeUninit::<xlib::XWindowAttributes>::uninit();
            assert!((XLIB.XGetWindowAttributes)(display, window, attrs.as_mut_ptr()) != 0);
            window_attrs = attrs.assume_init();
            assert!(window_attrs.depth == 24 || window_attrs.depth == 32);

            frame_buffer = vec![0; (window_attrs.width * window_attrs.height * 4) as usize];

            pixmap = (XLIB.XCreatePixmap)(
                display,
                window,
                window_attrs.width as u32,
                window_attrs.height as u32,
                window_attrs.depth as u32,
            );
            image = (XLIB.XCreateImage)(
                display,
                window_attrs.visual,
                window_attrs.depth as u32,
                xlib::ZPixmap,
                0,
                frame_buffer.as_mut_ptr() as *mut _,
                window_attrs.width as u32,
                window_attrs.height as u32,
                32,
                0,
            );
            assert!(!image.is_null());

            (XLIB.XPutImage)(
                display,
                pixmap,
                (*window_attrs.screen).default_gc,
                image,
                0,
                0,
                0,
                0,
                window_attrs.width as u32,
                window_attrs.height as u32,
            );

            (XLIB.XCopyArea)(
                display,
                pixmap,
                window,
                (*window_attrs.screen).default_gc,
                0,
                0,
                window_attrs.width as u32,
                window_attrs.height as u32,
                0,
                0,
            );
        }
        XlibHandle {
            window,
            display,
            window_attrs,
            frame_buffer,
            pixmap,
            image,
        }
    }

    pub fn frame_buffer(&mut self) -> &mut [u8] {
        &mut self.frame_buffer
    }

    pub fn redraw(&mut self) {
        unsafe {
            (XLIB.XPutImage)(
                self.display,
                self.pixmap,
                (*self.window_attrs.screen).default_gc,
                self.image,
                0,
                0,
                0,
                0,
                self.window_attrs.width as u32,
                self.window_attrs.height as u32,
            );

            (XLIB.XCopyArea)(
                self.display,
                self.pixmap,
                self.window,
                (*self.window_attrs.screen).default_gc,
                0,
                0,
                self.window_attrs.width as u32,
                self.window_attrs.height as u32,
                0,
                0,
            );
        }
    }

    pub fn width(&self) -> u32 {
        self.window_attrs.width as u32
    }

    pub fn height(&self) -> u32 {
        self.window_attrs.height as u32
    }
}

impl Drop for XlibHandle {
    fn drop(&mut self) {
        unsafe {
            (XLIB.XFreePixmap)(self.display, self.pixmap);
        }
    }
}