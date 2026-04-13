mod activity_capture;
mod chat;
mod daily_summary;
mod enhance;
mod title;
mod tool;
mod transcript_patch;
mod types;
mod validate;

pub use activity_capture::*;
pub use chat::*;
pub use daily_summary::*;
pub use enhance::*;
pub use title::*;
pub use tool::*;
pub use transcript_patch::*;
pub use types::*;
pub use validate::*;

#[macro_export]
macro_rules! common_derives {
    ($item:item) => {
        #[derive(Clone, serde::Deserialize, serde::Serialize, specta::Type)]
        #[serde(rename_all = "camelCase")]
        $item
    };
}

common_derives! {
    pub enum EditableTemplate {
        EnhanceUser,
        TitleUser,
    }
}

common_derives! {
    pub enum Template {
        ActivityCaptureSystem(ActivityCaptureSystem),
        ActivityCaptureUser(Box<ActivityCaptureUser>),
        DailySummarySystem(DailySummarySystem),
        DailySummaryUser(Box<DailySummaryUser>),
        EnhanceSystem(EnhanceSystem),
        EnhanceUser(Box<EnhanceUser>),
        TitleSystem(TitleSystem),
        TitleUser(TitleUser),
        ChatSystem(ChatSystem),
        ContextBlock(ContextBlock),
        ToolSearchSessions(ToolSearchSessions),
        TranscriptPatchSystem(TranscriptPatchSystem),
        TranscriptPatchUser(Box<TranscriptPatchUser>),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    AskamaError(#[from] askama::Error),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("validation error: {0}")]
    ValidationError(ValidationError),
}

pub fn render(t: Template) -> Result<String, Error> {
    let value = match t {
        Template::ActivityCaptureSystem(t) => askama::Template::render(&t),
        Template::ActivityCaptureUser(t) => askama::Template::render(&*t),
        Template::DailySummarySystem(t) => askama::Template::render(&t),
        Template::DailySummaryUser(t) => askama::Template::render(&*t),
        Template::EnhanceSystem(t) => askama::Template::render(&t),
        Template::EnhanceUser(t) => askama::Template::render(&*t),
        Template::TitleSystem(t) => askama::Template::render(&t),
        Template::TitleUser(t) => askama::Template::render(&t),
        Template::ChatSystem(t) => askama::Template::render(&t),
        Template::ContextBlock(t) => askama::Template::render(&t),
        Template::ToolSearchSessions(t) => askama::Template::render(&t),
        Template::TranscriptPatchSystem(t) => askama::Template::render(&t),
        Template::TranscriptPatchUser(t) => askama::Template::render(&*t),
    }?;

    Ok(value)
}

pub fn template_source(template: EditableTemplate) -> &'static str {
    match template {
        EditableTemplate::EnhanceUser => include_str!("../assets/enhance.user.md.jinja"),
        EditableTemplate::TitleUser => include_str!("../assets/title.user.md.jinja"),
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;

    #[test]
    fn editable_template_source_matches_assets() {
        assert_eq!(
            template_source(EditableTemplate::EnhanceUser),
            include_str!("../assets/enhance.user.md.jinja")
        );
        assert_eq!(
            template_source(EditableTemplate::TitleUser),
            include_str!("../assets/title.user.md.jinja")
        );
    }
}
