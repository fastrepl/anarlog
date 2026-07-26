#[cfg(feature = "actual")]
mod actual;
#[cfg(feature = "actual")]
pub use actual::*;

#[cfg(not(feature = "actual"))]
mod mock;
#[cfg(not(feature = "actual"))]
pub use mock::*;

#[derive(Debug, Default)]
pub struct Segment {
    pub text: String,
    pub language: Option<String>,
    pub start: f64,
    pub end: f64,
    pub confidence: f32,
    pub meta: Option<serde_json::Value>,
}

impl Segment {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    pub fn start(&self) -> f64 {
        self.start
    }

    pub fn end(&self) -> f64 {
        self.end
    }

    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    pub fn confidence(&self) -> f32 {
        self.confidence
    }

    pub fn meta(&self) -> Option<serde_json::Value> {
        self.meta.clone()
    }
}
