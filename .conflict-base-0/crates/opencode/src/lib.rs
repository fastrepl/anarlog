mod error;
mod events;
mod exec;
mod handoff;
mod health;
mod options;
mod plugin;
mod session;

pub use error::Error;
pub use events::{Event, EventStream, Input, RunStreamedResult, SessionTurn, UserInput};
pub use handoff::open_project_deeplink;
pub use health::{
    AuthStatus as HealthAuthStatus, HealthCheck, HealthStatus, health_check,
    health_check_with_options,
};
pub use options::{OpencodeOptions, SessionOptions, TurnOptions};
pub use plugin::{
    has_char_plugin, is_char_plugin, plugin_path, plugins_dir, remove_plugin, write_plugin,
};
pub use session::{Opencode, Session};
