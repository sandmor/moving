use super::*;
use crate::{error::*, event::*};

impl Connection {
    pub fn poll_event(&self) -> Result<Option<Event>, OSError> {
        todo!();
    }
}
