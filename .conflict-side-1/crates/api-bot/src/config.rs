use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct BotConfig {
    pub recall_api_key: String,
    /// Publicly reachable base URL for this API server, used to build webhook URLs.
    /// e.g. "https://api.hyprnote.com"
    pub public_url: String,
    /// Publicly accessible MP4 URL played by the onboarding bot.
    pub demo_video_url: String,
}
