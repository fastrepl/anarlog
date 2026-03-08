use std::sync::{Arc, Mutex};

use llm_proxy::{AnalyticsReporter, GenerationEvent};

#[derive(Default, Clone)]
pub struct MockAnalytics {
    events: Arc<Mutex<Vec<GenerationEvent>>>,
}

impl AnalyticsReporter for MockAnalytics {
    fn report_generation(
        &self,
        event: GenerationEvent,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        let events = self.events.clone();
        Box::pin(async move {
            events.lock().unwrap().push(event);
        })
    }
}

impl MockAnalytics {
    pub fn captured_events(&self) -> Vec<GenerationEvent> {
        self.events.lock().unwrap().clone()
    }
}

pub fn simple_message(content: &str) -> serde_json::Value {
    serde_json::json!({
        "messages": [{"role": "user", "content": content}],
        "max_tokens": 10
    })
}

pub fn stream_request(content: &str) -> serde_json::Value {
    serde_json::json!({
        "messages": [{"role": "user", "content": content}],
        "stream": true,
        "max_tokens": 10
    })
}
