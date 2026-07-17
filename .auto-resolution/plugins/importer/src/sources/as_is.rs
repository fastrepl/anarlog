use crate::types::{
    Collection, EnhancedNote, Human, Organization, Session, SessionParticipant, Tag, TagMapping,
    Template, Transcript,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AsIsData {
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub transcripts: Vec<Transcript>,
    #[serde(default)]
    pub humans: Vec<Human>,
    #[serde(default)]
    pub organizations: Vec<Organization>,
    #[serde(default)]
    pub participants: Vec<SessionParticipant>,
    #[serde(default)]
    pub templates: Vec<Template>,
    #[serde(default)]
    pub enhanced_notes: Vec<EnhancedNote>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub tag_mappings: Vec<TagMapping>,
}

pub fn load_data(path: &Path) -> Result<Collection, crate::Error> {
    if !path.exists() {
        return Ok(Collection::default());
    }
    let content = std::fs::read_to_string(path)?;
    let data: AsIsData = serde_json::from_str(&content)?;
    Ok(Collection {
        sessions: data.sessions,
        transcripts: data.transcripts,
        humans: data.humans,
        organizations: data.organizations,
        participants: data.participants,
        templates: data.templates,
        enhanced_notes: data.enhanced_notes,
        tags: data.tags,
        tag_mappings: data.tag_mappings,
    })
}
