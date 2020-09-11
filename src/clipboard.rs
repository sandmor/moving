use crate::{error::OSError, platform};

pub use mime;

pub fn load(media_type: mime::Mime) -> Result<Option<Vec<u8>>, OSError> {
    platform::clipboard::load(media_type)
}

pub fn store(media_type: mime::Mime, data: &[u8]) -> Result<(), OSError> {
    platform::clipboard::store(media_type, data)
}
