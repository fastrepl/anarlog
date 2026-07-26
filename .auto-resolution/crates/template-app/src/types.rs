use crate::common_derives;

common_derives! {
    pub struct Transcript {
        pub segments: Vec<Segment>,
        pub started_at: Option<u64>,
        pub ended_at: Option<u64>,
    }
}

common_derives! {
    pub struct Segment {
        pub text: String,
        pub speaker: String,
    }
}

common_derives! {
    pub struct Event {
        pub name: String,
    }
}

common_derives! {
    pub struct Session {
        pub title: Option<String>,
        pub started_at: Option<String>,
        pub ended_at: Option<String>,
        pub event: Option<Event>,
    }
}

common_derives! {
    pub struct Participant {
        pub name: String,
        pub job_title: Option<String>,
    }
}

common_derives! {
    pub struct TemplateSection {
        pub title: String,
        pub description: Option<String>,
    }
}

common_derives! {
    pub struct EnhanceTemplate {
        pub title: String,
        pub description: Option<String>,
        pub sections: Vec<TemplateSection>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use askama::Template;
    use hypr_askama_utils::tpl_snapshot;

    #[derive(Template)]
    #[template(
        source = r#"{%- import "_macros.jinja" as macros -%}{{ macros::transcripts(transcripts=transcripts) }}"#,
        ext = "txt"
    )]
    struct TestTranscripts {
        transcripts: Vec<Transcript>,
    }

    tpl_snapshot!(
        test_macro_transcripts,
        TestTranscripts {
            transcripts: vec![
                Transcript {
                    segments: vec![Segment { speaker: "Alice".to_string(), text: "First meeting".to_string() }],
                    started_at: None,
                    ended_at: None,
                },
                Transcript {
                    segments: vec![Segment { speaker: "Bob".to_string(), text: "Second meeting".to_string() }],
                    started_at: None,
                    ended_at: None,
                },
            ],
        },
        @"

    Alice: First meeting
    Bob: Second meeting
    "
    );

    #[derive(Template)]
    #[template(
        source = r#"{%- import "_macros.jinja" as macros -%}{{ macros::participants(participants=participants) }}"#,
        ext = "txt"
    )]
    struct TestParticipants {
        participants: Vec<Participant>,
    }

    tpl_snapshot!(
        test_macro_participants,
        TestParticipants {
            participants: vec![
                Participant { name: "Alice".to_string(), job_title: Some("Engineer".to_string()) },
                Participant { name: "Bob".to_string(), job_title: None },
            ],
        },
        @"


    - Alice (Engineer)

    - Bob
    "
    );

    tpl_snapshot!(
        test_macro_participants_empty,
        TestParticipants {
            participants: vec![],
        },
        @""
    );

    #[derive(Template)]
    #[template(
        source = r#"{%- import "_macros.jinja" as macros -%}{{ macros::session_context(session=session) }}"#,
        ext = "txt"
    )]
    struct TestSessionContext {
        session: Option<Session>,
    }

    tpl_snapshot!(
        test_macro_session_context_with_event,
        TestSessionContext {
            session: Some(Session {
                title: Some("Team Sync".to_string()),
                started_at: Some("2025-01-01 10:00".to_string()),
                ended_at: Some("2025-01-01 11:00".to_string()),
                event: Some(Event { name: "Calendar Event".to_string() }),
            }),
        },
        @"

    Meeting: Team Sync
    Time: 2025-01-01 10:00 - 2025-01-01 11:00
    "
    );

    tpl_snapshot!(
        test_macro_session_context_without_event,
        TestSessionContext {
            session: Some(Session {
                title: Some("Quick Note".to_string()),
                started_at: None,
                ended_at: None,
                event: None,
            }),
        },
        @"

    Session: Quick Note
    "
    );

    tpl_snapshot!(
        test_macro_session_context_none,
        TestSessionContext {
            session: None,
        },
        @""
    );

    #[derive(Template)]
    #[template(
        source = r#"{%- import "_macros.jinja" as macros -%}{{ macros::participants_list(participants=participants) }}"#,
        ext = "txt"
    )]
    struct TestParticipantsList {
        participants: Vec<Participant>,
    }

    tpl_snapshot!(
        test_macro_participants_list,
        TestParticipantsList {
            participants: vec![
                Participant { name: "Alice".to_string(), job_title: Some("CEO".to_string()) },
                Participant { name: "Bob".to_string(), job_title: None },
            ],
        },
        @"

    Participants:
    - Alice (CEO)
      - Bob
    "
    );

    #[derive(Template)]
    #[template(
        source = r#"{%- import "_macros.jinja" as macros -%}{{ macros::template_numbered(template=template) }}"#,
        ext = "txt"
    )]
    struct TestTemplateNumbered {
        template: Option<EnhanceTemplate>,
    }

    tpl_snapshot!(
        test_macro_template_numbered_some,
        TestTemplateNumbered {
            template: Some(EnhanceTemplate {
                title: "Meeting Notes".to_string(),
                description: Some("Standard meeting format".to_string()),
                sections: vec![
                    TemplateSection { title: "Summary".to_string(), description: Some("Brief overview".to_string()) },
                    TemplateSection { title: "Action Items".to_string(), description: None },
                ],
            }),
        },
        @"


    # Summary Template

    Name: Meeting Notes
    Description: Standard meeting format

    Sections:
    1. Summary - Brief overview
    2. Action Items
    "
    );

    tpl_snapshot!(
        test_macro_template_numbered_none,
        TestTemplateNumbered {
            template: None,
        },
        @"


    # Instructions

    1. Analyze the content and decide the sections to use.
    2. Generate a well-formatted markdown summary.
    "
    );
}
