use super::*;
use crate::{error::*, event::*};
use mime::Mime;

impl Connection {
    pub fn store_on_clipboard(&self, media_type: Mime, data: &[u8]) -> Result<(), OSError> {
        todo!();
    }

    pub fn load_from_clipboard(&self, media_type: Mime) -> Result<Option<Vec<u8>>, OSError> {
        todo!();
    }
}
