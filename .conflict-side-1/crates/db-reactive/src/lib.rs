#![forbid(unsafe_code)]

mod error;
mod explain;
mod runtime;
mod schema;
mod subscriptions;
mod types;
mod watch;

pub use error::{Error, Result};
pub use explain::extract_dependencies;
pub use runtime::LiveQueryRuntime;
pub use schema::DependencyResolutionError;
pub use types::{DependencyAnalysis, DependencyTarget, QueryEventSink, SubscriptionRegistration};
