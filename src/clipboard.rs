use crate::{error::OSError, platform};

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum ClipboardDataKind {
    Utf8,
}

pub fn load(kind: ClipboardDataKind) -> Result<Option<Vec<u8>>, OSError> {
    platform::load_from_clipboard(kind)
}
