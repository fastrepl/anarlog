use crate::common_derives;

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "event-contact.system.md.jinja")]
    pub struct EventContactSystem {}
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "event-contact.user.md.jinja")]
    pub struct EventContactUser {
        pub title: Option<String>,
        pub description: Option<String>,
        pub candidates: Vec<EventContactCandidate>,
    }
}

common_derives! {
    pub struct EventContactCandidate {
        pub name: Option<String>,
        pub email: Option<String>,
        pub is_current_user: bool,
        pub is_organizer: bool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_askama_utils::tpl_snapshot;

    tpl_snapshot!(
        test_event_contact_system,
        EventContactSystem {},
        fixed_date = "2025-01-01",
        @r#"
    # General Instructions

    Extract contacts from calendar event metadata.

    # Rules

    - Return only people who likely attended the meeting and should be added to contacts.
    - Prefer matching hinted names to candidate participant emails.
    - Exclude the organizer, current user, meeting platforms, bots, companies, URLs, and labels.
    - If unsure, return an empty contacts array.
    "#);

    tpl_snapshot!(
        test_event_contact_user,
        EventContactUser {
            title: Some("Yongkyun (Daniel) Lee <> john".to_string()),
            description: Some("What:\nYongkyun (Daniel) Lee <> john".to_string()),
            candidates: vec![
                EventContactCandidate {
                    name: Some("John Jeong".to_string()),
                    email: Some("john@example.com".to_string()),
                    is_current_user: true,
                    is_organizer: true,
                },
                EventContactCandidate {
                    name: Some("yongkyun.daniel.lee@gmail.com".to_string()),
                    email: Some("yongkyun.daniel.lee@gmail.com".to_string()),
                    is_current_user: false,
                    is_organizer: false,
                },
            ],
        },
        @r#"
    # Event Metadata

    Title: Yongkyun (Daniel) Lee <> john

    Description:
    What:
    Yongkyun (Daniel) Lee <> john

    # Candidate Participant Emails

    - John Jeong <john@example.com> (current user, organizer)
    - yongkyun.daniel.lee@gmail.com <yongkyun.daniel.lee@gmail.com>
    Extract contacts from this event.
    "#);
}
