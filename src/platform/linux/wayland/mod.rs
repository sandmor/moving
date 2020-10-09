mod window;
pub use window::*;

use crate::{error::OSError, event::*, platform::{WindowPlatformData, WindowId}};
use parking_lot::{Mutex, RwLock};
use std::{sync::Arc, collections::BTreeMap};
use wayland_client::{
    event_enum,
    protocol::{
        wl_compositor::WlCompositor, wl_pointer, wl_seat::WlSeat, wl_shm::WlShm,
        wl_subcompositor::WlSubcompositor,
    },
    Display, EventQueue, Filter, GlobalManager, Main,
};
use wayland_protocols::xdg_shell::client::xdg_wm_base::XdgWmBase;

event_enum!(
    Events |
    Pointer => wl_pointer::WlPointer
);

pub struct Connection {
    display: Display,
    event_queue: Mutex<EventQueue>,
    events_sender: flume::Sender<Event>,
    events_receiver: flume::Receiver<Event>,
    shm: Main<WlShm>,
    compositor: Main<WlCompositor>,
    subcompositor: Main<WlSubcompositor>,
    xdg_wm_base: Main<XdgWmBase>,
    windows: RwLock<BTreeMap<WindowId, Arc<RwLock<WindowPlatformData>>>>,
    mouse_on_surface: RwLock<Option<(u32, f64, f64)>>,
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
        let subcompositor = globals.instantiate_exact::<WlSubcompositor>(1).unwrap();
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
        let filter_events_sender = events_sender.clone();
        // initialize a seat to retrieve pointer & keyboard events
        let common_filter = Filter::new(move |event, _, _| match event {
            Events::Pointer { event, .. } => match event {
                wl_pointer::Event::Enter {
                    surface,
                    surface_x,
                    surface_y,
                    ..
                } => {
                    filter_events_sender.send(Event::WindowEvent { window: WindowId::from_wayland(surface.as_ref().id()), event: WindowEvent::MouseEnter { x: surface_x, y: surface_y } }).unwrap();
                }
                wl_pointer::Event::Leave { surface, .. } => {
                    filter_events_sender.send(Event::WindowEvent { window: WindowId::from_wayland(surface.as_ref().id()), event: WindowEvent::MouseLeave { x: 0.0, y: 0.0 } }).unwrap();
                }
                wl_pointer::Event::Motion {
                    surface_x,
                    surface_y,
                    ..
                } => {
                    filter_events_sender.send(Event::WindowEvent { window: WindowId::from_wayland(0), event: WindowEvent::MouseMove { x: surface_x, y: surface_y } }).unwrap();
                }
                wl_pointer::Event::Button { button, state, .. } => {
                    println!("Button {} was {:?}.", button, state);
                }
                _ => {}
            },
        });

        let mut pointer_created = false;
        globals
            .instantiate_exact::<WlSeat>(1)
            .unwrap()
            .quick_assign(move |seat, event, _| {
                use wayland_client::protocol::wl_seat::{Capability, Event as SeatEvent};

                if let SeatEvent::Capabilities { capabilities } = event {
                    if !pointer_created && capabilities.contains(Capability::Pointer) {
                        // create the pointer only once
                        pointer_created = true;
                        seat.get_pointer().assign(common_filter.clone());
                    }
                }
            });

        Ok(Self {
            display,
            event_queue: Mutex::new(event_queue),
            events_sender,
            events_receiver,
            shm,
            compositor,
            subcompositor,
            xdg_wm_base,
            windows: RwLock::new(BTreeMap::new()),
            mouse_on_surface: RwLock::new(None),
        })
    }

    pub fn poll_event(&self) -> Result<Option<Event>, OSError> {
        if let Ok(mut event) = self.events_receiver.try_recv() {
            return match event {
                Event::WindowEvent { window, event: WindowEvent::MouseEnter { x, y } } => {
                    self.mouse_on_surface.write().replace((window.to_wayland(), x, y));
                    if self.windows.read().get(&window).is_some() {
                        Ok(Some(event))
                    }
                    else {
                        Ok(None)
                    }
                },
                Event::WindowEvent { window, event: WindowEvent::MouseLeave { ref mut x, ref mut y } } => {
                    match self.mouse_on_surface.read().as_ref() {
                        Some((surface, rx, ry)) if *surface == window.to_wayland() => {
                            *x = *rx;
                            *y = *ry;
                        },
                        _ => {
                            return Ok(None);
                        }
                    }
                    if self.windows.read().get(&window).is_some() {
                        Ok(Some(event))
                    }
                    else {
                        Ok(None)
                    }
                },
                Event::WindowEvent { ref mut window, event: WindowEvent::MouseMove { x, y } } => {
                    let surface = match self.mouse_on_surface.read().as_ref() {
                        Some((surface, _, _)) => *surface,
                        None => return Ok(None)
                    };
                    self.mouse_on_surface.write().replace((surface, x, y));
                    if self.windows.read().get(&WindowId::from_wayland(surface)).is_some() {
                        *window = WindowId::from_wayland(surface);
                        Ok(Some(event))
                    }
                    else {
                        Ok(None)
                    }
                },
                ev@_ => Ok(Some(ev))
            };
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
