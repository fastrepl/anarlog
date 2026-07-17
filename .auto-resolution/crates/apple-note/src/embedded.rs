use crate::proto::Note;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EmbeddedObjectType {
    Table,
    Image,
    PDF,
    Drawing,
    URL,
    Audio,
    Video,
    Document,
    Gallery,
    Hashtag,
    Mention,
    Link,
    Unknown,
}

impl EmbeddedObjectType {
    pub fn from_uti(uti: &str) -> Self {
        match uti {
            "com.apple.notes.table" => Self::Table,
            "com.apple.notes.ICTable" => Self::Table,
            "public.image" => Self::Image,
            "com.apple.drawing.2" => Self::Drawing,
            "public.url" => Self::URL,
            "public.audio" => Self::Audio,
            "public.movie" => Self::Video,
            "com.apple.paper" => Self::Document,
            "com.apple.notes.gallery" => Self::Gallery,
            "com.adobe.pdf" => Self::PDF,
            _ if uti.contains("image") => Self::Image,
            _ if uti.contains("video") => Self::Video,
            _ if uti.contains("audio") => Self::Audio,
            _ if uti.contains("pdf") => Self::PDF,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedObject {
    pub object_type: EmbeddedObjectType,
    pub uuid: String,
    pub type_uti: String,
}

impl EmbeddedObject {
    pub fn new(object_type: EmbeddedObjectType, uuid: String, type_uti: String) -> Self {
        Self {
            object_type,
            uuid,
            type_uti,
        }
    }
}

pub fn extract_embedded_objects(note: &Note) -> Vec<EmbeddedObject> {
    let mut objects = Vec::new();

    for attr_run in &note.attribute_run {
        if let Some(ref attachment_info) = attr_run.attachment_info {
            let uuid = attachment_info
                .attachment_identifier
                .clone()
                .unwrap_or_default();

            let type_uti = attachment_info.type_uti.clone().unwrap_or_default();

            let object_type = EmbeddedObjectType::from_uti(&type_uti);

            objects.push(EmbeddedObject::new(object_type, uuid, type_uti));
        }
    }

    objects
}
