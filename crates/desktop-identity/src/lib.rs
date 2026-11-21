//! Where the desktop app keeps its identity: data locations, keychain names, the device
//! fingerprint and the sign-in session. The shipping Tauri app and the native app derive them
//! through this crate, so an update from one to the other finds the same data.

pub mod auth;
pub mod channel;
mod chunked;
pub mod device;
pub mod e2ee;
pub mod paths;
pub mod secure_store;
pub mod session;
pub mod store_json;
pub mod takeover;
pub mod vault;
pub mod webview_storage;

pub use channel::Channel;
pub use paths::Locations;
pub use secure_store::{SecretSlot, SecureStore};
