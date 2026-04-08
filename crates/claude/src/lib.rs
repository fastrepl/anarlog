mod config;
mod error;
mod events;
mod exec;
mod health;
mod options;
mod session;

pub use config::{
    HookEntry, HookMatcher, HooksConfig, has_command_hook, read_settings, remove_command_hook,
    settings_path, upsert_command_hook, write_settings,
};
pub use error::Error;
pub use events::{ClaudeEvent, EventStream, RunStreamedResult, Turn, Usage};
pub use health::{AuthStatus as HealthAuthStatus, HealthCheck, HealthStatus, health_check};
pub use options::{ClaudeOptions, PermissionMode, SessionOptions, SettingSource, TurnOptions};
pub use session::{Claude, Session};

pub type ClaudeHealthCheckOptions = ClaudeOptions;

pub fn health_check_with_options(options: &ClaudeOptions) -> HealthCheck {
    health::health_check_with_options(options)
}
