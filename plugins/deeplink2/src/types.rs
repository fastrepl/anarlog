pub use anlg_deeplink_core::{
    AuthCallbackSearch, BillingRefreshSearch, DeepLink, IncomingDeepLink,
    IntegrationCallbackSearch, OnboardingDemoCompleteSearch, ShareOpenRequest,
};
use specta::Type;

#[derive(Debug, Clone, serde::Serialize, Type, tauri_specta::Event)]
pub struct DeepLinkEvent(pub DeepLink);

#[derive(Debug, Clone, serde::Serialize, Type, tauri_specta::Event)]
pub struct ShareOpenPendingEvent {
    pub pending_id: String,
}
