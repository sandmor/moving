use crate::Size;
use std::ptr::NonNull;

#[cfg(target_os = "linux")]
#[path = "linux/mod.rs"]
mod platform;

#[cfg(target_os = "redox")]
#[path = "redox/mod.rs"]
mod platform;

#[cfg(any(target_os = "linux", target_os = "redox"))]
pub use platform::*;

#[cfg(all(not(target_os = "linux"), not(target_os = "redox")))]
compile_error!("Platform not supported");
