use serde::Deserialize;

pub const DEEPLINKS: &str = include_str!("../tests/fixtures/deeplink_contract.json");
pub const CALLBACKS: &str = include_str!("../tests/fixtures/callback_contract.json");

#[derive(Debug, Deserialize)]
pub struct DeepLinkCase {
    pub name: String,
    pub url: String,
    pub expect: DeepLinkExpectation,
}

#[derive(Debug, Deserialize)]
pub struct DeepLinkExpectation {
    pub kind: String,
    pub path: Option<String>,
    pub redacted: String,
}

#[derive(Debug, Deserialize)]
pub struct CallbackCase {
    pub name: String,
    pub path: String,
    pub query: String,
    pub scheme: String,
    pub expect: CallbackExpectation,
}

#[derive(Debug, Deserialize)]
pub struct CallbackExpectation {
    pub ok: bool,
    pub path: Option<String>,
    pub html_contains: Vec<String>,
}

pub fn deeplink_cases() -> Vec<DeepLinkCase> {
    serde_json::from_str(DEEPLINKS).expect("valid deeplink contract fixture")
}

pub fn callback_cases() -> Vec<CallbackCase> {
    serde_json::from_str(CALLBACKS).expect("valid callback contract fixture")
}
