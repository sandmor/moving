use libc::{mmap, munmap, MAP_ANON, MAP_FAILED, MAP_PRIVATE, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::RwLock;
use std::{os::unix::io::AsRawFd, ptr::null_mut, sync::Arc};
use x11rb::{
    connection::Connection,
    protocol::{
        shm::{self, ConnectionExt as ShmConnectionExt},
        xproto::{self, ConnectionExt},
    },
    wrapper::ConnectionExt as WrapperConnectionExt,
    COPY_DEPTH_FROM_PARENT,
};
use crate::{error::OSError, window::{Window as MWindow, WindowBuilder, WindowInner}, platform::WindowId, Size};
use super::XCB;

#[derive(Debug)]
enum WindowBufferKind {
    Native {
        screen_depth: u8,
    },
    Shm(shm::Seg),
}

#[derive(Debug)]
pub struct WindowPlatform {
    buffer: &'static mut [u8],
    buffer_kind: WindowBufferKind,
    pixmap: xproto::Pixmap,
    gcontext: xproto::Gcontext,
    width: u16,
    height: u16,
    inner: Arc<RwLock<WindowInner>>,
}


pub fn create_window(builder: WindowBuilder) -> Result<MWindow, OSError> {
    let screen = &XCB.conn.setup().roots[XCB.screen_num];
    let win_id = XCB.conn.generate_id()?;
    let width = builder.width as u16;
    let height = builder.height as u16;

    let win_aux = xproto::CreateWindowAux::new()
        .win_gravity(xproto::Gravity::NorthWest)
        .event_mask(
            xproto::EventMask::Exposure
                | xproto::EventMask::StructureNotify
                | xproto::EventMask::NoEvent,
        );

    XCB.conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win_id,
        screen.root,
        0,
        0,
        width,
        height,
        0,
        xproto::WindowClass::InputOutput,
        0,
        &win_aux,
    )?;

    XCB.conn.change_property32(
        xproto::PropMode::Replace,
        win_id,
        XCB.wm_protocols,
        xproto::AtomEnum::ATOM,
        &[XCB.wm_delete_window],
    )?;

    let gc_aux = xproto::CreateGCAux::new().graphics_exposures(0);
    let gcontext = XCB.conn.generate_id()?;
    XCB.conn.create_gc(gcontext, win_id, &gc_aux)?;

    XCB.conn.map_window(win_id)?;
    XCB.conn.flush()?;

    let pixmap = XCB.conn.generate_id()?;

    let buffer;
    let buffer_kind;

    let frame_buffer_ptr;
    let frame_buffer_len;

    if XCB.shm {
        let segment_size = (width as u32) * (height as u32) * 4;
        let shmseg = XCB.conn.generate_id()?;
        let reply = XCB.conn
            .shm_create_segment(shmseg, segment_size, false)?
            .reply()?;
        let shm::CreateSegmentReply { shm_fd, .. } = reply;

        let addr = unsafe {
            mmap(
                null_mut(),
                segment_size as _,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                shm_fd.as_raw_fd(),
                0,
            )
        };

        if addr == MAP_FAILED {
            XCB.conn.shm_detach(shmseg)?;
            return Err(x11rb::errors::ConnectionError::InsufficientMemory.into());
        }

        frame_buffer_ptr = addr as *mut u8;
        frame_buffer_len = segment_size as usize;
        buffer = unsafe { std::slice::from_raw_parts_mut(frame_buffer_ptr, frame_buffer_len) };

        buffer_kind = WindowBufferKind::Shm(shmseg);

        if let Err(e) =
            XCB.conn.shm_create_pixmap(pixmap, win_id, width, height, screen.root_depth, shmseg, 0)
        {
            let _ = XCB.conn.shm_detach(shmseg);
            return Err(e.into());
        }
    } else {
        frame_buffer_len = (width as usize) * (height as usize) * 4;
        XCB.conn.create_pixmap(screen.root_depth, pixmap, win_id, width, height)?;
        let addr = unsafe {
            mmap(
                null_mut(),
                frame_buffer_len,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANON,
                0,
                0,
            )
        };

        if addr == MAP_FAILED {
            return Err(x11rb::errors::ConnectionError::InsufficientMemory.into());
        }

        frame_buffer_ptr = addr as *mut u8;

        buffer = unsafe { std::slice::from_raw_parts_mut(frame_buffer_ptr, frame_buffer_len) };

        buffer_kind = WindowBufferKind::Native {
            screen_depth: screen.root_depth,
        };
    }

    let inner = Arc::new(RwLock::new(WindowInner {
        size: Size::new(builder.width, builder.height),
        frame_buffer_ptr,
        frame_buffer_len,
    }));

    let window = Arc::new(RwLock::new(crate::platform::WindowPlatform::Xcb(WindowPlatform {
        buffer,
        buffer_kind,
        pixmap,
        gcontext,
        width,
        height,
        inner: inner.clone(),
    })));

    XCB.conn.flush()?;

    Ok(MWindow {
        id: WindowId::from_x11(win_id),
        inner,
        platform: window,
    })
}

pub fn destroy_window(win: &mut WindowPlatform) -> Result<(), OSError> {
    match win.buffer_kind {
        WindowBufferKind::Native { .. } => {}
        WindowBufferKind::Shm(shmseg) => {
            XCB.conn.shm_detach(shmseg)?;
        }
    }
    unsafe {
        munmap(win.buffer.as_mut_ptr() as *mut _, win.buffer.len());
    }
    XCB.conn.free_pixmap(win.pixmap)?;
    Ok(())
}

pub fn redraw_window(id: WindowId, platform: &WindowPlatform) {
    match platform.buffer_kind {
        WindowBufferKind::Native {
            screen_depth,
        } => {
            XCB.conn.put_image(
                xproto::ImageFormat::ZPixmap,
                platform.pixmap,
                //screen_gcontext,
                platform.gcontext,
                platform.width,
                platform.height,
                0,
                0,
                0,
                screen_depth,
                platform.buffer,
            )
            .unwrap();

            XCB.conn.copy_area(
                platform.pixmap,
                id.0,
                //screen_gcontext,
                platform.gcontext,
                0,
                0,
                0,
                0,
                platform.width,
                platform.height,
            )
            .unwrap();
        }
        WindowBufferKind::Shm(_) => {
            XCB.conn.copy_area(
                platform.pixmap,
                id.0,
                platform.gcontext,
                0,
                0,
                0,
                0,
                platform.width,
                platform.height,
            )
            .unwrap();
        }
    }
    XCB.conn.flush().unwrap();
}