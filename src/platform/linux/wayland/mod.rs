use crate::{error::OSError, event::*, platform::{WindowId, WindowPlatformData}, window as mwin, Size};
use libc::{mmap, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE};
use parking_lot::{Mutex, RwLock};
use slab::Slab;
use std::{
    os::unix::io::AsRawFd,
    ptr::{null_mut, NonNull},
    sync::Arc,
    collections::BTreeMap
};
use wayland_client::{
    protocol::{
        wl_compositor::WlCompositor,
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Display, EventQueue, GlobalManager, Main,
};
use wayland_protocols::xdg_shell::client::{xdg_wm_base::XdgWmBase, xdg_toplevel::XdgToplevel};

#[derive(Debug)]
pub struct Window {
    xdg_toplevel: Main<XdgToplevel>,
    pool: Main<WlShmPool>,
    surface: Main<WlSurface>,
    buf_x: i32,
    buf_y: i32,
    on_slab_offset: usize,
}

pub struct Connection {
    display: Display,
    event_queue: Mutex<EventQueue>,
    events_sender: flume::Sender<Event>,
    events_receiver: flume::Receiver<Event>,
    shm: Main<WlShm>,
    compositor: Main<WlCompositor>,
    xdg_wm_base: Main<XdgWmBase>,
    windows: RwLock<Slab<Arc<RwLock<WindowPlatformData>>>>
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
        event_queue
            .sync_roundtrip(&mut (), |_, _, _| unreachable!())
            .unwrap();

        let shm = globals.instantiate_exact::<WlShm>(1).unwrap();
        let compositor = globals.instantiate_exact::<WlCompositor>(1).unwrap();
        let xdg_wm_base = globals.instantiate_exact::<XdgWmBase>(1).unwrap();

        xdg_wm_base.quick_assign(|xdg_wm_base, event, _| {
            use wayland_protocols::xdg_shell::client::xdg_wm_base::Event;
            // This ping/pong mechanism is used by the wayland server to detect
            // unresponsive applications
            if let Event::Ping { serial } = event {
                xdg_wm_base.pong(serial);
            }
        });

        let (events_sender, events_receiver) = flume::unbounded();

        Ok(Self {
            display,
            event_queue: Mutex::new(event_queue),
            events_sender,
            events_receiver,
            shm,
            compositor,
            xdg_wm_base,
            windows: RwLock::new(Slab::new())
        })
    }

    pub fn create_window(
        &self,
        builder: mwin::WindowBuilder,
    ) -> Result<(u32, Arc<RwLock<WindowPlatformData>>, Arc<RwLock<mwin::PixelsBox>>), OSError> {
        let buf_x: i32 = builder.width as i32;
        let buf_y: i32 = builder.height as i32;

        let surface = self.compositor.create_surface();
        let xdg_surface = self.xdg_wm_base.get_xdg_surface(&surface);
        let xdg_toplevel = xdg_surface.get_toplevel();

        xdg_surface.quick_assign(|xdg_surface, event, _| {
            use wayland_protocols::xdg_shell::client::xdg_surface::Event;
            match event {
                Event::Configure { serial } => {
                    xdg_surface.ack_configure(serial);
                }
                _ => {}
            }
        });
        let top_level_ev_sender = self.events_sender.clone();
        xdg_toplevel.quick_assign(move |_, event, _| {
            use wayland_protocols::xdg_shell::client::xdg_toplevel::Event as WlEvent;
            let window = WindowId(0);
            match event {
                WlEvent::Configure { .. } => {}
                WlEvent::Close => {
                    top_level_ev_sender
                        .send(Event::WindowEvent {
                            window,
                            event: WindowEvent::CloseRequested,
                        })
                        .unwrap();
                }
                _ => {}
            }
        });
        surface.commit();

        let tmp = tempfile::tempfile().expect("Unable to create a tempfile.");
        tmp.set_len((buf_x * buf_y) as u64 * 4).unwrap();

        let pool = self.shm.create_pool(
            tmp.as_raw_fd(),   // RawFd to the tempfile serving as shared memory
            buf_x * buf_y * 4, // size in bytes of the shared memory (4 bytes per pixel)
        );
        let buffer = pool.create_buffer(
            0,                  // Start of the buffer in the pool
            buf_x,              // width of the buffer in pixels
            buf_y,              // height of the buffer in pixels
            (buf_x * 4) as i32, // number of bytes between the beginning of two consecutive lines
            Format::Argb8888,   // chosen encoding for the data
        );

        self.event_queue
            .lock()
            .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
            .unwrap();

        surface.attach(Some(&buffer), 0, 0);
        surface.commit();

        self.event_queue
            .lock()
            .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
            .unwrap();

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

        let window = Arc::new(RwLock::new(WindowPlatformData::Wayland(Window {
            xdg_toplevel,
            pool,
            surface,
            buf_x,
            buf_y,
            on_slab_offset: 0
        })));
        let id = self.windows.write().insert(window.clone());
        window.write().wayland_mut().on_slab_offset = id;
        Ok((id as u32, window, pixels_box))
    }

    pub fn redraw_window(&self, window: &Window) {
        window.surface.damage(0, 0, window.buf_x, window.buf_y);
        window.surface.commit();
    }

    pub fn destroy_window(&self, window: &mut Window) -> Result<(), OSError> {
        window.xdg_toplevel.destroy();
        self.windows.write().remove(window.on_slab_offset);
        Ok(())
    }

    pub fn poll_event(&self) -> Result<Option<Event>, OSError> {
        if let Ok(event) = self.events_receiver.try_recv() {
            return Ok(Some(event));
        }
        self.event_queue
            .lock()
            .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
            .unwrap();
        Ok(self.events_receiver.try_recv().ok())
    }
}

unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}
