use crate::{error::OSError, platform};

pub use mime;

pub fn load(media_type: mime::Mime) -> Result<Option<Vec<u8>>, OSError> {
    platform::load_from_clipboard(media_type)
}

pub fn store(media_type: mime::Mime, data: &[u8]) -> Result<(), OSError> {
    platform::store_on_clipboard(media_type, data)
}
