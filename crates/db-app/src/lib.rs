#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
pub mod human_cli;
#[cfg(feature = "cli")]
pub mod organization_cli;

mod aliases_ops;
mod aliases_types;
mod calendars_ops;
mod calendars_types;
mod chat_messages_ops;
mod chat_messages_types;
mod connections_ops;
mod connections_types;
mod events_ops;
mod events_types;
mod folders_ops;
mod folders_types;
mod humans_ops;
mod humans_types;
mod meeting_participants_ops;
mod meeting_participants_types;
mod meetings_ops;
mod meetings_types;
mod notes_ops;
mod notes_types;
mod organizations_ops;
mod organizations_types;
mod settings_ops;
mod slack_ops;
mod slack_types;
mod threads_messages_ops;
mod threads_messages_types;
mod timeline_ops;
mod timeline_types;
mod transcript_ops;
mod transcript_types;
mod users_ops;
mod users_types;
mod visibility_ops;

pub use aliases_ops::*;
pub use aliases_types::*;
pub use calendars_ops::*;
pub use calendars_types::*;
pub use chat_messages_ops::*;
pub use chat_messages_types::*;
pub use connections_ops::*;
pub use connections_types::*;
pub use events_ops::*;
pub use events_types::*;
pub use folders_ops::*;
pub use folders_types::*;
pub use humans_ops::*;
pub use humans_types::*;
pub use meeting_participants_ops::*;
pub use meeting_participants_types::*;
pub use meetings_ops::*;
#[allow(unused_imports)]
pub use meetings_types::*;
pub use notes_ops::*;
pub use notes_types::*;
pub use organizations_ops::*;
pub use organizations_types::*;
pub use settings_ops::*;
pub use slack_ops::*;
pub use slack_types::*;
pub use threads_messages_ops::*;
pub use threads_messages_types::*;
pub use timeline_ops::*;
pub use timeline_types::*;
pub use transcript_ops::*;
pub use transcript_types::*;
pub use users_ops::*;
pub use users_types::*;
pub use visibility_ops::*;

use sqlx::SqlitePool;

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

#[cfg(feature = "cli")]
#[derive(Debug)]
pub struct CrudCliError {
    pub action: &'static str,
    pub message: String,
}

#[cfg(feature = "cli")]
impl CrudCliError {
    pub fn db(action: &'static str, e: sqlx::Error) -> Self {
        Self {
            action,
            message: e.to_string(),
        }
    }

    pub fn not_found(noun: &str, id: &str) -> Self {
        Self {
            action: "query",
            message: format!("{noun} '{id}' not found"),
        }
    }
}

#[cfg(feature = "cli")]
impl std::fmt::Display for CrudCliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} failed: {}", self.action, self.message)
    }
}

#[cfg(feature = "cli")]
impl std::error::Error for CrudCliError {}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;
    use hypr_transcript::{FinalizedWord, SpeakerHintData, WordState};

    // https://docs.sqlitecloud.io/docs/sqlite-sync-best-practices
    mod sync_compat {
        use super::*;

        // PRAGMA table_info returns: (cid, name, type, notnull, dflt_value, pk)
        type PragmaRow = (i32, String, String, i32, Option<String>, i32);

        async fn table_names(pool: &sqlx::SqlitePool) -> Vec<String> {
            sqlx::query_as::<_, (String,)>(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_sqlx%' AND name NOT LIKE '%_fts%'",
            )
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.0)
            .collect()
        }

        async fn table_info(pool: &sqlx::SqlitePool, table: &str) -> Vec<PragmaRow> {
            sqlx::query_as::<_, PragmaRow>(&format!("PRAGMA table_info('{}')", table))
                .fetch_all(pool)
                .await
                .unwrap()
        }

        #[tokio::test]
        async fn primary_keys_are_text_not_null() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            for table in &table_names(db.pool()).await {
                let cols = table_info(db.pool(), table).await;
                let pks: Vec<_> = cols.iter().filter(|c| c.5 != 0).collect();
                assert!(!pks.is_empty(), "{table}: no primary key");
                for pk in &pks {
                    assert_eq!(
                        pk.2.to_uppercase(),
                        "TEXT",
                        "{table}.{}: pk must be TEXT, got {}",
                        pk.1,
                        pk.2
                    );
                    assert_ne!(pk.3, 0, "{table}.{}: pk must be NOT NULL", pk.1);
                }
            }
        }

        #[tokio::test]
        async fn not_null_columns_have_defaults() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            let mut violations = vec![];
            for table in &table_names(db.pool()).await {
                for col in &table_info(db.pool(), table).await {
                    let (_, ref name, _, notnull, ref dflt, pk) = *col;
                    if pk != 0 || notnull == 0 {
                        continue;
                    }
                    if dflt.is_none() {
                        violations.push(format!("{table}.{name}"));
                    }
                }
            }

            assert!(
                violations.is_empty(),
                "NOT NULL non-PK columns without DEFAULT: {}",
                violations.join(", ")
            );
        }
    }

    #[tokio::test]
    async fn roundtrip_words_and_hints() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "sess-1";
        insert_meeting(db.pool(), sid, None).await.unwrap();

        let meeting = get_meeting(db.pool(), sid).await.unwrap().unwrap();
        assert_eq!(meeting.id, sid);
        assert!(meeting.title.is_none());
        assert_eq!(meeting.user_id, "");
        assert_eq!(meeting.visibility, "public");
        assert!(meeting.folder_id.is_none());

        update_meeting(db.pool(), sid, Some("My Title"))
            .await
            .unwrap();
        let meeting = get_meeting(db.pool(), sid).await.unwrap().unwrap();
        assert_eq!(meeting.title.as_deref(), Some("My Title"));

        let delta = TranscriptDeltaPersist {
            new_words: vec![
                FinalizedWord {
                    id: "w1".into(),
                    text: "hello".into(),
                    start_ms: 0,
                    end_ms: 500,
                    channel: 0,
                    state: WordState::Final,
                },
                FinalizedWord {
                    id: "w2".into(),
                    text: "world".into(),
                    start_ms: 500,
                    end_ms: 1000,
                    channel: 0,
                    state: WordState::Pending,
                },
            ],
            hints: vec![PersistableSpeakerHint {
                word_id: "w1".into(),
                data: SpeakerHintData::ProviderSpeakerIndex {
                    speaker_index: 0,
                    provider: Some("deepgram".into()),
                    channel: Some(0),
                },
            }],
            replaced_ids: vec![],
        };
        apply_delta(db.pool(), sid, &delta).await.unwrap();

        let words = load_words(db.pool(), sid).await.unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].id, "w1");
        assert_eq!(words[0].text, "hello");
        assert_eq!(words[1].state, WordState::Pending);

        let hints = load_hints(db.pool(), sid).await.unwrap();
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].word_id, "w1");
        match &hints[0].data {
            SpeakerHintData::ProviderSpeakerIndex {
                speaker_index,
                provider,
                ..
            } => {
                assert_eq!(*speaker_index, 0);
                assert_eq!(provider.as_deref(), Some("deepgram"));
            }
            _ => panic!("expected ProviderSpeakerIndex"),
        }
    }

    #[tokio::test]
    async fn replacement_removes_old_words() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "sess-2";
        insert_meeting(db.pool(), sid, None).await.unwrap();

        let delta1 = TranscriptDeltaPersist {
            new_words: vec![FinalizedWord {
                id: "w1".into(),
                text: "helo".into(),
                start_ms: 0,
                end_ms: 500,
                channel: 0,
                state: WordState::Pending,
            }],
            hints: vec![PersistableSpeakerHint {
                word_id: "w1".into(),
                data: SpeakerHintData::UserSpeakerAssignment {
                    human_id: "user-a".into(),
                },
            }],
            replaced_ids: vec![],
        };
        apply_delta(db.pool(), sid, &delta1).await.unwrap();

        let delta2 = TranscriptDeltaPersist {
            new_words: vec![FinalizedWord {
                id: "w1-corrected".into(),
                text: "hello".into(),
                start_ms: 0,
                end_ms: 500,
                channel: 0,
                state: WordState::Final,
            }],
            hints: vec![],
            replaced_ids: vec!["w1".into()],
        };
        apply_delta(db.pool(), sid, &delta2).await.unwrap();

        let words = load_words(db.pool(), sid).await.unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].id, "w1-corrected");
        assert_eq!(words[0].text, "hello");
        assert_eq!(words[0].state, WordState::Final);

        let hints = load_hints(db.pool(), sid).await.unwrap();
        assert!(hints.is_empty());
    }

    #[tokio::test]
    async fn chat_message_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "chat-sess-1";
        insert_meeting(db.pool(), sid, None).await.unwrap();

        insert_chat_message(db.pool(), "m1", sid, "user", "hello")
            .await
            .unwrap();
        insert_chat_message(db.pool(), "m2", sid, "assistant", "hi there")
            .await
            .unwrap();

        let messages = load_chat_messages(db.pool(), sid).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, "m1");
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].id, "m2");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "hi there");
    }

    #[tokio::test]
    async fn human_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_human(
            db.pool(),
            "h1",
            "Alice",
            "alice@example.com",
            "",
            "Engineer",
        )
        .await
        .unwrap();

        let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
        assert_eq!(human.name, "Alice");
        assert_eq!(human.email, "alice@example.com");
        assert_eq!(human.job_title, "Engineer");
        assert_eq!(human.user_id, "");
        assert!(human.linked_user_id.is_none());

        update_human(
            db.pool(),
            "h1",
            Some("Alice B"),
            None,
            None,
            None,
            Some("notes"),
        )
        .await
        .unwrap();
        let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
        assert_eq!(human.name, "Alice B");
        assert_eq!(human.email, "alice@example.com");
        assert_eq!(human.memo, "notes");

        let all = list_humans(db.pool()).await.unwrap();
        assert_eq!(all.len(), 1);

        delete_human(db.pool(), "h1").await.unwrap();
        assert!(get_human(db.pool(), "h1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn organization_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_organization(db.pool(), "org1", "Acme")
            .await
            .unwrap();

        let org = get_organization(db.pool(), "org1").await.unwrap().unwrap();
        assert_eq!(org.name, "Acme");
        assert_eq!(org.user_id, "");

        update_organization(db.pool(), "org1", Some("Acme Inc"))
            .await
            .unwrap();
        let org = get_organization(db.pool(), "org1").await.unwrap().unwrap();
        assert_eq!(org.name, "Acme Inc");

        insert_human(db.pool(), "h1", "Bob", "", "org1", "")
            .await
            .unwrap();

        delete_organization(db.pool(), "org1").await.unwrap();
        assert!(get_organization(db.pool(), "org1").await.unwrap().is_none());

        let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
        assert_eq!(human.org_id, "");
    }

    #[tokio::test]
    async fn meeting_participant_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_meeting(db.pool(), "s1", None).await.unwrap();
        insert_human(db.pool(), "h1", "Alice", "", "", "")
            .await
            .unwrap();
        insert_human(db.pool(), "h2", "Bob", "", "", "")
            .await
            .unwrap();

        add_meeting_participant(db.pool(), "s1", "h1", "manual")
            .await
            .unwrap();
        add_meeting_participant(db.pool(), "s1", "h2", "auto")
            .await
            .unwrap();

        let participants = list_meeting_participants(db.pool(), "s1").await.unwrap();
        assert_eq!(participants.len(), 2);

        let meetings = list_meetings_by_human(db.pool(), "h1").await.unwrap();
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].meeting_id, "s1");

        remove_meeting_participant(db.pool(), "s1", "h1")
            .await
            .unwrap();
        let participants = list_meeting_participants(db.pool(), "s1").await.unwrap();
        assert_eq!(participants.len(), 1);
        assert_eq!(participants[0].human_id, "h2");

        delete_human(db.pool(), "h2").await.unwrap();
        let participants = list_meeting_participants(db.pool(), "s1").await.unwrap();
        assert!(participants.is_empty());
    }

    #[tokio::test]
    async fn note_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "note-sess-1";
        insert_meeting(db.pool(), sid, None).await.unwrap();

        insert_note(db.pool(), "n1", sid, "memo", "", "my memo")
            .await
            .unwrap();
        insert_note(db.pool(), "n2", sid, "summary", "", "my summary")
            .await
            .unwrap();

        let notes = list_notes_by_meeting(db.pool(), sid).await.unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].user_id, "");
        assert_eq!(notes[0].visibility, "public");

        let memo = get_note_by_meeting_and_kind(db.pool(), sid, "memo")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(memo.content, "my memo");

        let summary = get_note_by_meeting_and_kind(db.pool(), sid, "summary")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(summary.content, "my summary");

        update_note(db.pool(), "n1", "updated memo").await.unwrap();
        let memo = get_note_by_meeting_and_kind(db.pool(), sid, "memo")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(memo.content, "updated memo");

        delete_notes_by_meeting(db.pool(), sid).await.unwrap();
        let notes = list_notes_by_meeting(db.pool(), sid).await.unwrap();
        assert!(notes.is_empty());
    }

    #[tokio::test]
    async fn user_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_user(db.pool(), "u1", "Alice").await.unwrap();

        let user = get_user(db.pool(), "u1").await.unwrap().unwrap();
        assert_eq!(user.id, "u1");
        assert_eq!(user.name, "Alice");
        assert!(!user.created_at.is_empty());

        update_user(db.pool(), "u1", Some("Alice B")).await.unwrap();
        let user = get_user(db.pool(), "u1").await.unwrap().unwrap();
        assert_eq!(user.name, "Alice B");

        assert!(get_user(db.pool(), "nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn thread_and_message_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_meeting(db.pool(), "s1", None).await.unwrap();

        insert_thread(db.pool(), "t1", "u1", Some("s1"), "Chat about code")
            .await
            .unwrap();
        insert_thread(db.pool(), "t2", "u1", None, "Standalone thread")
            .await
            .unwrap();

        let thread = get_thread(db.pool(), "t1").await.unwrap().unwrap();
        assert_eq!(thread.title, "Chat about code");
        assert_eq!(thread.meeting_id.as_deref(), Some("s1"));
        assert_eq!(thread.visibility, "public");

        let thread2 = get_thread(db.pool(), "t2").await.unwrap().unwrap();
        assert!(thread2.meeting_id.is_none());

        update_thread(db.pool(), "t1", Some("Updated title"))
            .await
            .unwrap();
        let thread = get_thread(db.pool(), "t1").await.unwrap().unwrap();
        assert_eq!(thread.title, "Updated title");

        let by_meeting = list_threads_by_meeting(db.pool(), "s1").await.unwrap();
        assert_eq!(by_meeting.len(), 1);
        assert_eq!(by_meeting[0].id, "t1");

        insert_message(
            db.pool(),
            "msg1",
            "u1",
            "t1",
            "user",
            r#"[{"type":"text","text":"hello"}]"#,
        )
        .await
        .unwrap();
        insert_message(
            db.pool(),
            "msg2",
            "u1",
            "t1",
            "assistant",
            r#"[{"type":"text","text":"hi there"}]"#,
        )
        .await
        .unwrap();

        let msgs = load_messages(db.pool(), "t1").await.unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[0].visibility, "public");

        update_message(
            db.pool(),
            "msg1",
            Some(r#"[{"type":"text","text":"updated"}]"#),
        )
        .await
        .unwrap();
        let msgs = load_messages(db.pool(), "t1").await.unwrap();
        assert!(msgs[0].parts.contains("updated"));

        delete_messages_by_thread(db.pool(), "t1").await.unwrap();
        let msgs = load_messages(db.pool(), "t1").await.unwrap();
        assert!(msgs.is_empty());

        delete_thread(db.pool(), "t1").await.unwrap();
        assert!(get_thread(db.pool(), "t1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn event_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        upsert_event(
            db.pool(),
            "e1",
            "u1",
            "cal1",
            "track1",
            "Standup",
            "2026-03-19T09:00:00Z",
            "2026-03-19T09:30:00Z",
            "Room A",
            "https://meet.example.com/123",
            "Daily standup",
            "",
            "",
            false,
            false,
            "[]",
            "{}",
        )
        .await
        .unwrap();

        let event = get_event(db.pool(), "e1").await.unwrap().unwrap();
        assert_eq!(event.title, "Standup");
        assert_eq!(event.calendar_id, "cal1");
        assert_eq!(event.location, "Room A");
        assert!(!event.has_recurrence_rules);
        assert!(!event.is_all_day);

        let by_cal = list_events_by_calendar(db.pool(), "cal1").await.unwrap();
        assert_eq!(by_cal.len(), 1);

        let in_range =
            list_events_in_range(db.pool(), "2026-03-19T00:00:00Z", "2026-03-20T00:00:00Z")
                .await
                .unwrap();
        assert_eq!(in_range.len(), 1);

        let out_of_range =
            list_events_in_range(db.pool(), "2026-03-20T00:00:00Z", "2026-03-21T00:00:00Z")
                .await
                .unwrap();
        assert!(out_of_range.is_empty());

        // upsert overwrites
        upsert_event(
            db.pool(),
            "e1",
            "u1",
            "cal1",
            "track1",
            "Updated Standup",
            "2026-03-19T09:00:00Z",
            "2026-03-19T09:30:00Z",
            "",
            "",
            "",
            "",
            "",
            false,
            false,
            "[]",
            "{}",
        )
        .await
        .unwrap();
        let event = get_event(db.pool(), "e1").await.unwrap().unwrap();
        assert_eq!(event.title, "Updated Standup");

        delete_event(db.pool(), "e1").await.unwrap();
        assert!(get_event(db.pool(), "e1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn folder_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_folder(db.pool(), "f1", "u1", "Work", None)
            .await
            .unwrap();
        insert_folder(db.pool(), "f2", "u1", "Projects", Some("f1"))
            .await
            .unwrap();

        let folder = get_folder(db.pool(), "f1").await.unwrap().unwrap();
        assert_eq!(folder.name, "Work");
        assert!(folder.parent_id.is_none());

        let child = get_folder(db.pool(), "f2").await.unwrap().unwrap();
        assert_eq!(child.parent_id.as_deref(), Some("f1"));

        let all = list_folders(db.pool()).await.unwrap();
        assert_eq!(all.len(), 2);

        update_folder(db.pool(), "f1", Some("Work Stuff"), None)
            .await
            .unwrap();
        let folder = get_folder(db.pool(), "f1").await.unwrap().unwrap();
        assert_eq!(folder.name, "Work Stuff");

        // reparent
        update_folder(db.pool(), "f2", None, Some(None))
            .await
            .unwrap();
        let child = get_folder(db.pool(), "f2").await.unwrap().unwrap();
        assert!(child.parent_id.is_none());

        delete_folder(db.pool(), "f1").await.unwrap();
        assert!(get_folder(db.pool(), "f1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn set_meeting_visibility_propagates() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "vis-sess-1";
        insert_meeting(db.pool(), sid, None).await.unwrap();

        let delta = TranscriptDeltaPersist {
            new_words: vec![FinalizedWord {
                id: "w1".into(),
                text: "hello".into(),
                start_ms: 0,
                end_ms: 500,
                channel: 0,
                state: WordState::Final,
            }],
            hints: vec![PersistableSpeakerHint {
                word_id: "w1".into(),
                data: SpeakerHintData::ProviderSpeakerIndex {
                    speaker_index: 0,
                    provider: None,
                    channel: None,
                },
            }],
            replaced_ids: vec![],
        };
        apply_delta(db.pool(), sid, &delta).await.unwrap();
        insert_note(db.pool(), "n1", sid, "memo", "", "note content")
            .await
            .unwrap();

        let meeting = get_meeting(db.pool(), sid).await.unwrap().unwrap();
        assert_eq!(meeting.visibility, "public");

        set_meeting_visibility(db.pool(), sid, "private")
            .await
            .unwrap();

        let meeting = get_meeting(db.pool(), sid).await.unwrap().unwrap();
        assert_eq!(meeting.visibility, "private");

        let note = get_note_by_meeting_and_kind(db.pool(), sid, "memo")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(note.visibility, "private");

        // verify words visibility via raw query
        let vis =
            sqlx::query_as::<_, (String,)>("SELECT visibility FROM words WHERE meeting_id = ?")
                .bind(sid)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(vis.0, "private");

        let vis = sqlx::query_as::<_, (String,)>(
            "SELECT visibility FROM speaker_hints WHERE meeting_id = ?",
        )
        .bind(sid)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(vis.0, "private");
    }

    #[tokio::test]
    async fn alias_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_human(db.pool(), "h1", "Alice", "alice@example.com", "", "")
            .await
            .unwrap();

        upsert_alias(
            db.pool(),
            "a1",
            "h1",
            "slack",
            "U12345",
            "T999",
            "alice",
            "confirmed",
        )
        .await
        .unwrap();

        let found = get_alias_by_external(db.pool(), "slack", "U12345", "T999")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, "a1");
        assert_eq!(found.human_id, "h1");
        assert_eq!(found.display_name, "alice");
        assert_eq!(found.confidence, "confirmed");

        upsert_alias(
            db.pool(),
            "a2",
            "h1",
            "email",
            "alice@example.com",
            "",
            "Alice",
            "confirmed",
        )
        .await
        .unwrap();

        let aliases = list_aliases_by_human(db.pool(), "h1").await.unwrap();
        assert_eq!(aliases.len(), 2);

        delete_alias(db.pool(), "a1").await.unwrap();
        let aliases = list_aliases_by_human(db.pool(), "h1").await.unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].provider, "email");
    }

    #[tokio::test]
    async fn resolve_or_create_alias_auto_creates() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let alias = resolve_or_create_alias(db.pool(), "slack", "U999", "T1", "Bob Slack")
            .await
            .unwrap();
        assert_eq!(alias.confidence, "auto");
        assert_eq!(alias.display_name, "Bob Slack");

        let human = get_human(db.pool(), &alias.human_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(human.name, "Bob Slack");

        let alias2 = resolve_or_create_alias(db.pool(), "slack", "U999", "T1", "Bob Slack")
            .await
            .unwrap();
        assert_eq!(alias2.id, alias.id);
    }

    #[tokio::test]
    async fn slack_entity_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        upsert_slack_team(db.pool(), "st1", "conn1", "T123", "Acme Workspace")
            .await
            .unwrap();
        let team = get_slack_team(db.pool(), "st1").await.unwrap().unwrap();
        assert_eq!(team.team_name, "Acme Workspace");
        assert_eq!(team.team_id, "T123");

        upsert_slack_channel(db.pool(), "sc1", "st1", "C456", "general", "channel", false)
            .await
            .unwrap();
        let channel = get_slack_channel(db.pool(), "sc1").await.unwrap().unwrap();
        assert_eq!(channel.name, "general");
        assert!(!channel.is_external);

        upsert_slack_thread(
            db.pool(),
            "sth1",
            "sc1",
            "1234567890.123456",
            "2026-03-19T10:00:00Z",
            "2026-03-19T10:05:00Z",
            3,
        )
        .await
        .unwrap();
        let thread = get_slack_thread(db.pool(), "sth1").await.unwrap().unwrap();
        assert_eq!(thread.message_count, 3);
        assert_eq!(thread.thread_ts, "1234567890.123456");

        insert_human(db.pool(), "h1", "Alice", "", "", "")
            .await
            .unwrap();
        upsert_alias(
            db.pool(),
            "a1",
            "h1",
            "slack",
            "U111",
            "T123",
            "alice",
            "confirmed",
        )
        .await
        .unwrap();

        insert_slack_message(
            db.pool(),
            "sm1",
            "sth1",
            "sc1",
            "a1",
            "Hello world",
            "1234567890.123456",
            "{}",
        )
        .await
        .unwrap();
        let messages = list_slack_messages_by_thread(db.pool(), "sth1")
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "Hello world");
        assert_eq!(messages[0].alias_id, "a1");

        upsert_slack_thread_participant(db.pool(), "stp1", "sth1", "a1")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn note_on_human_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_human(db.pool(), "h1", "Alice", "", "", "")
            .await
            .unwrap();

        insert_note_on_entity(
            db.pool(),
            "n1",
            "human",
            "h1",
            "memo",
            "About Alice",
            "She likes Rust",
        )
        .await
        .unwrap();

        let notes = list_notes_by_entity(db.pool(), "human", "h1")
            .await
            .unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].entity_type, "human");
        assert_eq!(notes[0].entity_id, "h1");
        assert_eq!(notes[0].title, "About Alice");
        assert_eq!(notes[0].content, "She likes Rust");
        assert_eq!(notes[0].meeting_id, "");
    }

    #[tokio::test]
    async fn timeline_view_query() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        // Setup human
        insert_human(db.pool(), "h1", "Alice", "", "", "")
            .await
            .unwrap();

        // Meeting
        insert_meeting(db.pool(), "s1", None).await.unwrap();
        update_meeting(db.pool(), "s1", Some("Weekly Standup"))
            .await
            .unwrap();
        add_meeting_participant(db.pool(), "s1", "h1", "auto")
            .await
            .unwrap();

        // Slack thread
        upsert_slack_team(db.pool(), "st1", "conn1", "T1", "Workspace")
            .await
            .unwrap();
        upsert_slack_channel(db.pool(), "sc1", "st1", "C1", "#general", "channel", false)
            .await
            .unwrap();
        upsert_slack_thread(
            db.pool(),
            "sth1",
            "sc1",
            "123.456",
            "2026-03-19T11:00:00Z",
            "2026-03-19T11:05:00Z",
            2,
        )
        .await
        .unwrap();
        upsert_alias(
            db.pool(),
            "a1",
            "h1",
            "slack",
            "U1",
            "T1",
            "alice",
            "confirmed",
        )
        .await
        .unwrap();
        upsert_slack_thread_participant(db.pool(), "stp1", "sth1", "a1")
            .await
            .unwrap();

        // Note on human
        insert_note_on_entity(
            db.pool(),
            "n1",
            "human",
            "h1",
            "memo",
            "Note title",
            "content",
        )
        .await
        .unwrap();

        // Query timeline
        let timeline = list_timeline_by_human(db.pool(), "h1").await.unwrap();
        assert_eq!(timeline.len(), 3);

        let types: Vec<&str> = timeline.iter().map(|t| t.source_type.as_str()).collect();
        assert!(types.contains(&"meeting"));
        assert!(types.contains(&"slack"));
        assert!(types.contains(&"note"));

        let meeting = timeline
            .iter()
            .find(|t| t.source_type == "meeting")
            .unwrap();
        assert_eq!(meeting.source_id, "s1");
        assert_eq!(meeting.title, "Weekly Standup");

        let slack = timeline.iter().find(|t| t.source_type == "slack").unwrap();
        assert_eq!(slack.source_id, "sth1");
        assert_eq!(slack.title, "#general");

        let note = timeline.iter().find(|t| t.source_type == "note").unwrap();
        assert_eq!(note.source_id, "n1");
        assert_eq!(note.title, "Note title");
    }

    #[tokio::test]
    async fn meeting_event_link_copies_participants() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        // Create event
        upsert_event(
            db.pool(),
            "e1",
            "u1",
            "cal1",
            "track1",
            "Standup",
            "2026-03-19T09:00:00Z",
            "2026-03-19T09:30:00Z",
            "",
            "",
            "",
            "",
            "",
            false,
            false,
            "[]",
            "{}",
        )
        .await
        .unwrap();

        // Create humans
        insert_human(db.pool(), "h1", "Alice", "alice@example.com", "", "")
            .await
            .unwrap();
        insert_human(db.pool(), "h2", "Bob", "bob@example.com", "", "")
            .await
            .unwrap();

        // Add event participants (one with human_id, one without)
        upsert_event_participant(
            db.pool(),
            "ep1",
            "e1",
            Some("h1"),
            "alice@example.com",
            "Alice",
            false,
            false,
            "u1",
        )
        .await
        .unwrap();
        upsert_event_participant(
            db.pool(),
            "ep2",
            "e1",
            Some("h2"),
            "bob@example.com",
            "Bob",
            false,
            false,
            "u1",
        )
        .await
        .unwrap();
        upsert_event_participant(
            db.pool(),
            "ep3",
            "e1",
            None,
            "unknown@example.com",
            "Unknown",
            false,
            false,
            "u1",
        )
        .await
        .unwrap();

        // Create meeting linked to event
        insert_meeting(db.pool(), "s1", Some("e1")).await.unwrap();

        let meeting = get_meeting(db.pool(), "s1").await.unwrap().unwrap();
        assert_eq!(meeting.event_id.as_deref(), Some("e1"));

        // Copy event participants to meeting
        let copied = copy_event_participants_to_meeting(db.pool(), "s1", "e1")
            .await
            .unwrap();
        assert_eq!(copied, 2);

        let participants = list_meeting_participants(db.pool(), "s1").await.unwrap();
        assert_eq!(participants.len(), 2);

        let sources: Vec<&str> = participants.iter().map(|p| p.source.as_str()).collect();
        assert!(sources.iter().all(|s| *s == "event"));

        let human_ids: Vec<&str> = participants.iter().map(|p| p.human_id.as_str()).collect();
        assert!(human_ids.contains(&"h1"));
        assert!(human_ids.contains(&"h2"));
    }

    mod crud_macro {
        use super::*;

        #[tokio::test]
        async fn get_returns_none_for_missing_id() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            assert!(get_human(db.pool(), "nonexistent").await.unwrap().is_none());
            assert!(
                get_organization(db.pool(), "nonexistent")
                    .await
                    .unwrap()
                    .is_none()
            );
        }

        #[tokio::test]
        async fn bool_fields_roundtrip() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            insert_human(db.pool(), "h1", "Alice", "", "", "")
                .await
                .unwrap();

            let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
            assert!(!human.pinned, "pinned should default to false");

            sqlx::query("UPDATE humans SET pinned = 1 WHERE id = 'h1'")
                .execute(db.pool())
                .await
                .unwrap();

            let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
            assert!(human.pinned, "pinned should be true after setting to 1");
        }

        #[tokio::test]
        async fn option_string_fields_roundtrip() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            insert_human(db.pool(), "h1", "Alice", "", "", "")
                .await
                .unwrap();

            let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
            assert!(
                human.linked_user_id.is_none(),
                "linked_user_id should be None by default"
            );

            sqlx::query("UPDATE humans SET linked_user_id = 'u42' WHERE id = 'h1'")
                .execute(db.pool())
                .await
                .unwrap();

            let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
            assert_eq!(human.linked_user_id.as_deref(), Some("u42"));
        }

        #[tokio::test]
        async fn update_coalesce_preserves_unset_fields() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            insert_organization(db.pool(), "org1", "Acme")
                .await
                .unwrap();

            // update with None should preserve existing value
            update_organization(db.pool(), "org1", None).await.unwrap();
            let org = get_organization(db.pool(), "org1").await.unwrap().unwrap();
            assert_eq!(org.name, "Acme");

            // update with Some should change value
            update_organization(db.pool(), "org1", Some("NewCo"))
                .await
                .unwrap();
            let org = get_organization(db.pool(), "org1").await.unwrap().unwrap();
            assert_eq!(org.name, "NewCo");
        }

        #[tokio::test]
        async fn list_returns_ordered_results() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            insert_organization(db.pool(), "org1", "Alpha")
                .await
                .unwrap();
            // insert a small delay via explicit created_at so ordering is deterministic
            sqlx::query("UPDATE organizations SET created_at = '2025-01-01' WHERE id = 'org1'")
                .execute(db.pool())
                .await
                .unwrap();

            insert_organization(db.pool(), "org2", "Beta")
                .await
                .unwrap();
            sqlx::query("UPDATE organizations SET created_at = '2025-06-01' WHERE id = 'org2'")
                .execute(db.pool())
                .await
                .unwrap();

            let orgs = list_organizations(db.pool()).await.unwrap();
            assert_eq!(orgs.len(), 2);
            // list_order = "created_at DESC" so Beta (newer) should come first
            assert_eq!(orgs[0].name, "Beta");
            assert_eq!(orgs[1].name, "Alpha");
        }

        #[tokio::test]
        async fn generated_insert_and_update_multi_field() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            insert_human(db.pool(), "h1", "Alice", "a@b.com", "org1", "Eng")
                .await
                .unwrap();

            let h = get_human(db.pool(), "h1").await.unwrap().unwrap();
            assert_eq!(h.name, "Alice");
            assert_eq!(h.email, "a@b.com");
            assert_eq!(h.org_id, "org1");
            assert_eq!(h.job_title, "Eng");
            assert_eq!(h.memo, "");

            // partial update: only change memo and job_title
            update_human(
                db.pool(),
                "h1",
                None,
                None,
                None,
                Some("CTO"),
                Some("great"),
            )
            .await
            .unwrap();

            let h = get_human(db.pool(), "h1").await.unwrap().unwrap();
            assert_eq!(h.name, "Alice", "name should be unchanged");
            assert_eq!(h.email, "a@b.com", "email should be unchanged");
            assert_eq!(h.job_title, "CTO");
            assert_eq!(h.memo, "great");
        }
    }
}
