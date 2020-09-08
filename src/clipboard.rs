use crate::{error::OSError, platform};

pub use mime;

pub fn load(media_type: mime::Mime) -> Result<Option<Vec<u8>>, OSError> {
    platform::load_from_clipboard(media_type)
}
