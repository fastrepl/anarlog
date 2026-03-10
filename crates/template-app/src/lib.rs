mod chat;
mod enhance;
mod title;
mod tool;
mod transcript_patch;
mod types;
mod validate;

pub use chat::*;
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
    pub enum Template {
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
