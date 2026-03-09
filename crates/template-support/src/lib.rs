#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub user_id: String,
    pub email: Option<String>,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub stripe_customer_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub platform: String,
    pub arch: String,
    pub os_version: String,
    pub app_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

#[derive(askama::Template)]
#[template(path = "bug_report.md.jinja", escape = "none")]
struct BugReportBody<'a> {
    description: &'a str,
    platform: &'a str,
    arch: &'a str,
    os_version: &'a str,
    app_version: &'a str,
    source: &'a str,
}

#[derive(askama::Template)]
#[template(path = "feature_request.md.jinja", escape = "none")]
struct FeatureRequestBody<'a> {
    description: &'a str,
    platform: &'a str,
    arch: &'a str,
    os_version: &'a str,
    app_version: &'a str,
    source: &'a str,
}

#[derive(askama::Template)]
#[template(path = "log_analysis.md.jinja", escape = "none")]
struct LogAnalysisComment<'a> {
    summary_section: &'a str,
    tail: &'a str,
}

#[derive(askama::Template, Default)]
#[template(path = "support_chat.md.jinja", escape = "none")]
struct SupportChatPrompt;

#[derive(askama::Template)]
#[template(path = "support_context.md.jinja", escape = "none")]
struct SupportContextBlock<'a> {
    account: Option<&'a AccountInfo>,
    device: &'a DeviceInfo,
}

#[derive(Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SupportTemplate {
    SupportContext(SupportContext),
    BugReport(BugReport),
    FeatureRequest(FeatureRequest),
    LogAnalysis(LogAnalysis),
}

#[derive(Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SupportContext {
    pub account: Option<AccountInfo>,
    pub device: DeviceInfo,
}

#[derive(Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BugReport {
    pub description: String,
    pub platform: String,
    pub arch: String,
    pub os_version: String,
    pub app_version: String,
    pub source: String,
}

#[derive(Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FeatureRequest {
    pub description: String,
    pub platform: String,
    pub arch: String,
    pub os_version: String,
    pub app_version: String,
    pub source: String,
}

#[derive(Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LogAnalysis {
    pub summary_section: String,
    pub tail: String,
}

pub fn render(t: SupportTemplate) -> Result<String, askama::Error> {
    match t {
        SupportTemplate::SupportContext(t) => askama::Template::render(&SupportContextBlock {
            account: t.account.as_ref(),
            device: &t.device,
        }),
        SupportTemplate::BugReport(t) => askama::Template::render(&BugReportBody {
            description: &t.description,
            platform: &t.platform,
            arch: &t.arch,
            os_version: &t.os_version,
            app_version: &t.app_version,
            source: &t.source,
        }),
        SupportTemplate::FeatureRequest(t) => askama::Template::render(&FeatureRequestBody {
            description: &t.description,
            platform: &t.platform,
            arch: &t.arch,
            os_version: &t.os_version,
            app_version: &t.app_version,
            source: &t.source,
        }),
        SupportTemplate::LogAnalysis(t) => askama::Template::render(&LogAnalysisComment {
            summary_section: &t.summary_section,
            tail: &t.tail,
        }),
    }
}

pub fn render_support_chat() -> Result<String, askama::Error> {
    askama::Template::render(&SupportChatPrompt)
}
