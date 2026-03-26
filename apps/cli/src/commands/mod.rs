pub mod transcribe;

#[cfg(feature = "task")]
pub mod claude;
#[cfg(feature = "task")]
pub mod codex;
#[cfg(feature = "desktop")]
pub mod export;
#[cfg(feature = "desktop")]
pub mod humans;
#[cfg(feature = "desktop")]
pub mod meetings;
#[cfg(feature = "task")]
pub mod opencode;
#[cfg(feature = "desktop")]
pub mod orgs;

#[cfg(feature = "standalone")]
pub mod bug;
#[cfg(feature = "standalone")]
pub mod desktop;
#[cfg(feature = "standalone")]
pub mod hello;
#[cfg(feature = "standalone")]
pub mod model;
#[cfg(feature = "standalone")]
pub mod record;
