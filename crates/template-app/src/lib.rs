mod activity_capture;
mod chat;
mod daily_summary;
mod enhance;
mod event_contact;
mod title;
mod tool;
mod transcript_patch;
mod types;
mod validate;

pub use activity_capture::*;
pub use chat::*;
pub use daily_summary::*;
pub use enhance::*;
pub use event_contact::*;
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
        EnhanceSystem,
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
        EventContactSystem(EventContactSystem),
        EventContactUser(EventContactUser),
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
    #[error(transparent)]
    JinjaError(#[from] minijinja::Error),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("validation error: {0}")]
    ValidationError(ValidationError),
}

pub fn render(t: Template) -> Result<String, Error> {
    match t {
        Template::EnhanceSystem(t) => render_enhance_system(&t),
        Template::ActivityCaptureSystem(t) => Ok(askama::Template::render(&t)?),
        Template::ActivityCaptureUser(t) => Ok(askama::Template::render(&*t)?),
        Template::DailySummarySystem(t) => Ok(askama::Template::render(&t)?),
        Template::DailySummaryUser(t) => Ok(askama::Template::render(&*t)?),
        Template::EnhanceUser(t) => Ok(askama::Template::render(&*t)?),
        Template::EventContactSystem(t) => Ok(askama::Template::render(&t)?),
        Template::EventContactUser(t) => Ok(askama::Template::render(&t)?),
        Template::TitleSystem(t) => Ok(askama::Template::render(&t)?),
        Template::TitleUser(t) => Ok(askama::Template::render(&t)?),
        Template::ChatSystem(t) => Ok(askama::Template::render(&t)?),
        Template::ContextBlock(t) => Ok(askama::Template::render(&t)?),
        Template::ToolSearchSessions(t) => Ok(askama::Template::render(&t)?),
        Template::TranscriptPatchSystem(t) => Ok(askama::Template::render(&t)?),
        Template::TranscriptPatchUser(t) => Ok(askama::Template::render(&*t)?),
    }
}

pub fn template_source(template: EditableTemplate) -> &'static str {
    match template {
        EditableTemplate::EnhanceSystem => include_str!("../assets/enhance.system.md.jinja"),
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
            template_source(EditableTemplate::EnhanceSystem),
            include_str!("../assets/enhance.system.md.jinja")
        );
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
