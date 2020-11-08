use std::{cell::RefCell, cmp::Ordering, rc::Rc};
use mime::Mime;
use wayland_client::{protocol::wl_data_offer::WlDataOffer, Main};

#[derive(Debug)]
pub struct DataOffer {
    wl_data_offer: Main<WlDataOffer>,
    mime_types: Rc<RefCell<Vec<Mime>>>
}

impl DataOffer {
    pub fn from_wl(wl_data_offer: Main<WlDataOffer>) -> Self {
        let mime_types = Rc::new(RefCell::new(Vec::<Mime>::new()));
        let events_mime_types = mime_types.clone();
        wl_data_offer.quick_assign(move |_, event, _| {
            use wayland_client::protocol::wl_data_offer::Event;
            match event {
                Event::Offer { mime_type } => {
                    println!("{}", mime_type);
                    let mime_type = match mime_type.parse() {
                        Ok(mime_type) => mime_type,
                        Err(_) => {
                            return;
                        }
                    };
                    events_mime_types.borrow_mut().push(mime_type);
                },
                _ => {}
            }
        });
        Self { wl_data_offer, mime_types }
    }
}

impl Ord for DataOffer {
    fn cmp(&self, other: &DataOffer) -> Ordering {
        self.wl_data_offer.as_ref().id().cmp(&other.wl_data_offer.as_ref().id())
    }
}
impl PartialOrd for DataOffer {
    fn partial_cmp(&self, other: &DataOffer) -> Option<Ordering> {
        self.wl_data_offer.as_ref().id().partial_cmp(&other.wl_data_offer.as_ref().id())
    }
}

impl Eq for DataOffer {}
impl PartialEq for DataOffer {
    fn eq(&self, other: &DataOffer) -> bool {
        self.wl_data_offer.as_ref().id() == other.wl_data_offer.as_ref().id()
    }
}