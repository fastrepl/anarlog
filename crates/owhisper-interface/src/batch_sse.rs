use crate::{InferenceProgress, batch, common_derives, stream};

pub const EVENT_NAME: &str = "batch";

common_derives! {
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum BatchSseMessage {
        Progress { progress: InferenceProgress },
        Segment { response: stream::StreamResponse },
        Result { response: batch::Response },
        Error { error: String, detail: String },
    }
}
