pub struct Participant {
    pub name: String,
    pub job_title: Option<String>,
}

#[derive(askama::Template)]
#[template(path = "chat.context.md.jinja")]
pub struct ChatContext {
    pub meeting_id: String,
    pub title: Option<String>,
    pub created_at: Option<String>,
    pub participants: Vec<Participant>,
    pub memo: Option<String>,
    pub summary: Option<String>,
    pub transcript_text: Option<String>,
}

impl ChatContext {
    pub fn render(&self) -> Result<String, askama::Error> {
        askama::Template::render(self)
    }
}
