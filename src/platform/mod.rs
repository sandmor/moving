#[cfg(target_os = "linux")]
#[path = "linux/mod.rs"]
mod platform;

#[cfg(target_os = "linux")]
pub use platform::*;

#[cfg(not(target_os = "linux"))]
compile_error!("Platform not supported");
