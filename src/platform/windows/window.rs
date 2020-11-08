use atomic::Atomic;
use parking_lot::RwLock;
use std::{
    ffi::OsStr,
    io::Error,
    iter,
    os::windows::ffi::OsStrExt,
    ptr::{self, NonNull},
    sync::{Arc, atomic::AtomicPtr},
};
use winapi::um::{
    libloaderapi::GetModuleHandleW,
    winuser::{
        CreateWindowExW, DefWindowProcW, PostQuitMessage, RegisterClassW, GetDpiForWindow, GetDC, GetDesktopWindow, CS_HREDRAW, CS_OWNDC,
        CS_VREDRAW, WNDCLASSW, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_SYSMENU, WS_VISIBLE,
    },
    wingdi::{BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, CreateDIBSection}
};

use super::*;
use crate::{error::OSError, window as mwin, Size, dpi::LogicalSize, surface::{self, Surface}};

fn win32_string(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(iter::once(0))
        .collect()
}

#[derive(Debug)]
pub struct WindowPlatformData {
    buffer: Vec<u8>,
}

impl Connection {
    pub fn create_window(&self, builder: mwin::WindowBuilder) -> Result<mwin::Window, OSError> {
        let name = "com.moving.window";
        let title = "Window";
        let name = win32_string(name);
        let title = win32_string(title);

        unsafe {
            let hinstance = GetModuleHandleW(ptr::null_mut());

            let wnd_class = WNDCLASSW {
                style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(events::window_proc),
                hInstance: hinstance,
                lpszClassName: name.as_ptr(),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hIcon: ptr::null_mut(),
                hCursor: ptr::null_mut(),
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null_mut(),
            };

            let error_code = RegisterClassW(&wnd_class);
            assert!(error_code != 0, "failed to register the window class");

            let handle = CreateWindowExW(
                0,
                name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_MINIMIZEBOX | WS_SYSMENU,
                0,
                0,
                builder.width as i32,
                builder.height as i32,
                ptr::null_mut(),
                ptr::null_mut(),
                hinstance,
                ptr::null_mut(),
            );

            if handle.is_null() {
                return Err(Error::last_os_error().into());
            }
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: mem::size_of::<BITMAPINFOHEADER>(),
                    biWidth: builder.width as i32,
                    biHeight: -builder.height as i32,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    .. Default::default()
                },
                .. Default::default()
            };
            let h_desk_dc = GetDC(GetDesktopWindow());
            CreateDIBSection(h_desk_dc, &bmi, DIB_RGB_COLORS, , 0, 0);
            let frame_buffer_len = (builder.width as usize) * (builder.height as usize) * 4;
            let mut buffer = Vec::with_capacity(frame_buffer_len);

            let frame_buffer_ptr: NonNull<u8> =
                NonNull::new(buffer.as_mut_slice().as_mut_ptr()).unwrap();

            let shared = Arc::new((AtomicPtr::new(buffer.as_mut_slice().as_mut_ptr()), Atomic::new((surface::SharedData { buffer_len: buffer.len(), width: builder.width as u32, height: builder.height as u32 }))));

            let surface = Surface::new(builder.surface_format, shared);

            let platform_data = Arc::new(RwLock::new(WindowPlatformData { buffer }));

            let id = WindowId::from_hwnd(handle);
            self.windows.write().insert(id, platform_data.clone());
            Ok(mwin::Window {
                id,
                surface,
                logical_size: Arc::new(Atomic::new(LogicalSize {
                    w: builder.width,
                    h: builder.height,
                })),
                dpi: Arc::new(Atomic::new(GetDpiForWindow(handle) as f64)),
                platform_data,
            })
        }
    }

    pub fn destroy_window(&self, window: &mut WindowPlatformData) -> Result<(), OSError> {
        Ok(())
    }

    pub fn redraw_window(&self, window: &mwin::Window) {}
}
