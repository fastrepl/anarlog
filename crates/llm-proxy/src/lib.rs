mod analytics;
mod config;
mod env;
mod handler;
pub mod model;
mod openapi;
pub mod provider;
mod types;

pub const CHAR_TASK_HEADER: &str = "x-char-task";

pub use analytics::{AnalyticsReporter, GenerationEvent};
pub use anlg_analytics::{AuthenticatedUserId, DeviceFingerprint};
pub use config::*;
pub use env::{ApiKey, Env};
pub use handler::{ModelsResponse, chat_completions_router, router};
pub use model::{
    ANARLOG_MODELS, CharTask, MODEL_KEY_AUDIO, MODEL_KEY_DEFAULT, MODEL_KEY_TOOL_CALLING,
    ModelContext, ModelResolver, SELECTABLE_MODELS, SelectableModel, StaticModelResolver,
};
pub use openapi::openapi;
