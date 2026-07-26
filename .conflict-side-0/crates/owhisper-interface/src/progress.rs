use crate::common_derives;

common_derives! {
    pub enum InferencePhase {
        #[serde(rename = "transcribing")]
        Transcribing,
        #[serde(rename = "prefill")]
        Prefill,
        #[serde(rename = "decoding")]
        Decoding,
    }
}

common_derives! {
    pub struct InferenceProgress {
        pub percentage: f64,
        pub partial_text: Option<String>,
        pub phase: InferencePhase,
    }
}
