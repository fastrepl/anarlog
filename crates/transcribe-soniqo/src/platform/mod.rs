#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod macos;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(crate) use macos::*;

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
mod unsupported;
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub(crate) use unsupported::*;
