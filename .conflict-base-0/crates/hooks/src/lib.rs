mod config;
mod error;
mod event;
mod naming;
mod runner;

pub use config::{HookDefinition, HooksConfig};
pub use error::{Error, Result};
pub use event::{AfterListeningStoppedArgs, BeforeListeningStartedArgs, HookArgs, HookEvent};
pub use naming::cli_flag;
pub use runner::{HookResult, run_hooks_for_event};
