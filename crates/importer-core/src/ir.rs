macro_rules! common_derives {
    ($item:item) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        $item
    };
}

#[derive(Debug, Default)]
pub struct Collection {
    pub sessions: Vec<Session>,
    pub transcripts: Vec<Transcript>,
    pub humans: Vec<Human>,
    pub organizations: Vec<Organization>,
    pub participants: Vec<SessionParticipant>,
    pub templates: Vec<Template>,
    pub enhanced_notes: Vec<EnhancedNote>,
    pub tags: Vec<Tag>,
    pub tag_mappings: Vec<TagMapping>,
}

impl std::fmt::Display for Collection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "sessions: {}", self.sessions.len())?;
        writeln!(f, "transcripts: {}", self.transcripts.len())?;
        writeln!(f, "humans: {}", self.humans.len())?;
        writeln!(f, "organizations: {}", self.organizations.len())?;
        writeln!(f, "participants: {}", self.participants.len())?;
        writeln!(f, "templates: {}", self.templates.len())?;
        writeln!(f, "enhanced_notes: {}", self.enhanced_notes.len())?;
        writeln!(f, "tags: {}", self.tags.len())?;
        writeln!(f, "tag_mappings: {}", self.tag_mappings.len())?;

        if let Some(s) = self.sessions.first() {
            writeln!(f, "\n[First Session]")?;
            writeln!(f, "  id: {}", s.id)?;
            writeln!(f, "  title: {}", s.title)?;
            writeln!(f, "  created_at: {}", s.created_at)?;
            if let Some(ref md) = s.raw_md {
                let preview: String = md.chars().take(100).collect();
                writeln!(f, "  raw_md: {}...", preview)?;
            }
        }

        if let Some(t) = self.transcripts.first() {
            writeln!(f, "\n[First Transcript]")?;
            writeln!(f, "  id: {}", t.id)?;
            writeln!(f, "  session_id: {}", t.session_id)?;
            writeln!(f, "  started_at: {}", t.started_at)?;
            writeln!(f, "  words: {}", t.words.len())?;
            if let (Some(start), Some(end)) = (t.start_ms, t.end_ms) {
                let duration_secs = (end - start) / 1000.0;
                writeln!(
                    f,
                    "  duration: {:.1}s ({}ms - {}ms)",
                    duration_secs, start, end
                )?;
            }
            if !t.words.is_empty() {
                let first = &t.words[0];
                writeln!(
                    f,
                    "  first_word: {:?} (ch={}, {}ms-{}ms, speaker={:?})",
                    first.text.trim(),
                    first.channel,
                    first.start_ms.unwrap_or(0.0),
                    first.end_ms.unwrap_or(0.0),
                    first.speaker
                )?;
                let text: String = t.words.iter().take(20).map(|w| w.text.as_str()).collect();
                writeln!(f, "  preview: {}", text.trim())?;

                let unique_speakers: std::collections::HashSet<_> =
                    t.words.iter().filter_map(|w| w.speaker.as_ref()).collect();
                writeln!(f, "  unique_speakers: {:?}", unique_speakers)?;
            }
        }

        if let Some(h) = self.humans.first() {
            writeln!(f, "\n[First Human]")?;
            writeln!(f, "  id: {}", h.id)?;
            writeln!(f, "  name: {}", h.name)?;
            if let Some(ref email) = h.email {
                writeln!(f, "  email: {}", email)?;
            }
        }

        Ok(())
    }
}

common_derives! {
    pub struct Session {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub created_at: String,
        #[serde(default)]
        pub title: String,
        #[serde(default)]
        pub raw_md: Option<String>,
        #[serde(default)]
        pub enhanced_content: Option<String>,
        #[serde(default)]
        pub folder_id: Option<String>,
        #[serde(default)]
        pub event_id: Option<String>,
    }
}

common_derives! {
    pub struct Transcript {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub created_at: String,
        #[serde(default)]
        pub session_id: String,
        #[serde(default)]
        pub title: String,
        #[serde(default)]
        pub started_at: f64,
        #[serde(default)]
        pub ended_at: Option<f64>,
        #[serde(default)]
        pub start_ms: Option<f64>,
        #[serde(default)]
        pub end_ms: Option<f64>,
        #[serde(default)]
        pub words: Vec<Word>,
        #[serde(default)]
        pub speaker_hints: Vec<SpeakerHint>,
    }
}

common_derives! {
    pub struct SpeakerHint {
        pub word_id: String,
        #[serde(default)]
        pub hint_type: String,
        #[serde(default)]
        pub value: String,
    }
}

common_derives! {
    pub struct Word {
        pub id: String,
        #[serde(default)]
        pub text: String,
        #[serde(default)]
        pub start_ms: Option<f64>,
        #[serde(default)]
        pub end_ms: Option<f64>,
        #[serde(default)]
        pub channel: i64,
        #[serde(default)]
        pub speaker: Option<String>,
    }
}

common_derives! {
    pub struct Human {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub created_at: String,
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub email: Option<String>,
        #[serde(default)]
        pub org_id: Option<String>,
        #[serde(default)]
        pub job_title: Option<String>,
        #[serde(default)]
        pub linkedin_username: Option<String>,
    }
}

common_derives! {
    pub struct Organization {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub created_at: String,
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub description: Option<String>,
    }
}

common_derives! {
    pub struct SessionParticipant {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub session_id: String,
        #[serde(default)]
        pub human_id: String,
        #[serde(default)]
        pub source: String,
    }
}

common_derives! {
    pub struct Template {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub title: String,
        #[serde(default)]
        pub description: String,
        #[serde(default)]
        pub sections: Vec<TemplateSection>,
        #[serde(default)]
        pub tags: Vec<String>,
        #[serde(default)]
        pub context_option: Option<String>,
    }
}

common_derives! {
    pub struct TemplateSection {
        #[serde(default)]
        pub title: String,
        #[serde(default)]
        pub description: String,
    }
}

common_derives! {
    pub struct EnhancedNote {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub session_id: String,
        #[serde(default)]
        pub content: String,
        #[serde(default)]
        pub template_id: Option<String>,
        #[serde(default)]
        pub position: i32,
        #[serde(default)]
        pub title: String,
    }
}

common_derives! {
    pub struct Tag {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub name: String,
    }
}

common_derives! {
    pub struct TagMapping {
        pub id: String,
        #[serde(default)]
        pub user_id: String,
        #[serde(default)]
        pub tag_id: String,
        #[serde(default)]
        pub session_id: String,
    }
}
