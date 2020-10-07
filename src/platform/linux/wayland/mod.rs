use crate::{error::OSError, window as mwin, Size, event::*};
use libc::{MAP_SHARED, MAP_FAILED, mmap, PROT_READ, PROT_WRITE};
use parking_lot::{RwLock, Mutex};
use wayland_client::{Display, EventQueue, Main, GlobalManager, protocol::{wl_compositor::WlCompositor, wl_shm::{WlShm, Format}}};
use wayland_protocols::xdg_shell::client::xdg_wm_base::XdgWmBase;
use std::{io::Write, ptr::{null_mut, NonNull}, os::unix::io::AsRawFd, sync::Arc};

#[derive(Debug)]
pub struct Window {
}

pub struct Connection {
    display: Display,
    event_queue: Mutex<EventQueue>,
    events_receiver: flume::Receiver<Event>,
    shm: Main<WlShm>,
    compositor: Main<WlCompositor>,
    xdg_wm_base: Main<XdgWmBase>,
}

impl Connection {
    pub fn from_display(display: Display) -> Result<Self, OSError> {
        let mut event_queue = display.create_event_queue();

        let attached_display = (*display).clone().attach(event_queue.token());

        let globals = GlobalManager::new(&attached_display);

        // Make a synchronized roundtrip to the wayland server.
        //
        // When this returns it must be true that the server has already
        // sent us all available globals.
        event_queue.sync_roundtrip(&mut (), |_, _, _| unreachable!()).unwrap();

        let shm = globals.instantiate_exact::<WlShm>(1).unwrap();
        let compositor = globals.instantiate_exact::<WlCompositor>(1).unwrap();
        let xdg_wm_base = globals.instantiate_exact::<XdgWmBase>(1).unwrap();

        let (events_sender, events_receiver) = flume::unbounded();

        Ok(Self { display, event_queue: Mutex::new(event_queue), events_receiver, shm, compositor, xdg_wm_base })
    }

    pub fn create_window(
        &self,
        builder: mwin::WindowBuilder,
    ) -> Result<(u32, Window, Arc<RwLock<mwin::PixelsBox>>), OSError> {
        let buf_x: u32 = builder.width as u32;
        let buf_y: u32 = builder.height as u32;

        let mut tmp = tempfile::tempfile().expect("Unable to create a tempfile.");
        tmp.set_len((buf_x * buf_y) as u64 * 4).unwrap();

        let pool = self.shm.create_pool(
            tmp.as_raw_fd(),            // RawFd to the tempfile serving as shared memory
            (buf_x * buf_y * 4) as i32, // size in bytes of the shared memory (4 bytes per pixel)
        );
        let buffer = pool.create_buffer(
            0,                        // Start of the buffer in the pool
            buf_x as i32,             // width of the buffer in pixels
            buf_y as i32,             // height of the buffer in pixels
            (buf_x * 4) as i32,       // number of bytes between the beginning of two consecutive lines
            Format::Argb8888,         // chosen encoding for the data
        );

        let surface = self.compositor.create_surface();
        let xdg_surface = self.xdg_wm_base.get_xdg_surface(&surface);
        let top_level = xdg_surface.get_toplevel();
        surface.attach(Some(&buffer), 0, 0);
        surface.commit();

        self.event_queue.lock().sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ }).unwrap();

        let frame_buffer_len = (buf_x * buf_y) as usize * 4;

        let in_memory_addr = unsafe {
            mmap(
                null_mut(),
                frame_buffer_len,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                tmp.as_raw_fd(),
                0,
            )
        };
        assert_ne!(in_memory_addr, MAP_FAILED);

        let frame_buffer_ptr = NonNull::new(in_memory_addr as *mut u8).unwrap();

        let pixels_box = Arc::new(RwLock::new(mwin::PixelsBox::from_raw(
            Size::new(builder.width, builder.height),
            frame_buffer_ptr,
            frame_buffer_len,
        )));

        let window = Window {};
        Ok((0, window, pixels_box))
    }

    pub fn poll_event(&self) -> Result<Option<Event>, OSError> {
        if let Some(event) = self.events_receiver.try_recv().ok() {
            return Ok(Some(event));
        }
        self.event_queue.lock().dispatch_pending(&mut (), |_, _, _| { /* we ignore unfiltered messages */ }).unwrap();
        Ok(self.events_receiver.try_recv().ok())
    }
}

unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}