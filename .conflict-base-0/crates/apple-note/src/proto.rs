use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct Color {
    #[prost(float, required, tag = "1")]
    pub red: f32,
    #[prost(float, required, tag = "2")]
    pub green: f32,
    #[prost(float, required, tag = "3")]
    pub blue: f32,
    #[prost(float, required, tag = "4")]
    pub alpha: f32,
}

impl Color {
    pub fn red_hex_string(&self) -> String {
        format!("{:02X}", (self.red * 255.0) as u8)
    }

    pub fn green_hex_string(&self) -> String {
        format!("{:02X}", (self.green * 255.0) as u8)
    }

    pub fn blue_hex_string(&self) -> String {
        format!("{:02X}", (self.blue * 255.0) as u8)
    }

    pub fn full_hex_string(&self) -> String {
        format!(
            "#{}{}{}",
            self.red_hex_string(),
            self.green_hex_string(),
            self.blue_hex_string()
        )
    }
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct AttachmentInfo {
    #[prost(string, optional, tag = "1")]
    pub attachment_identifier: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub type_uti: Option<String>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct Font {
    #[prost(string, optional, tag = "1")]
    pub font_name: Option<String>,
    #[prost(float, optional, tag = "2")]
    pub point_size: Option<f32>,
    #[prost(int32, optional, tag = "3")]
    pub font_hints: Option<i32>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct Checklist {
    #[prost(bytes, required, tag = "1")]
    pub uuid: Vec<u8>,
    #[prost(int32, required, tag = "2")]
    pub done: i32,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct ParagraphStyle {
    #[prost(int32, optional, tag = "1", default = "-1")]
    pub style_type: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    pub alignment: Option<i32>,
    #[prost(int32, optional, tag = "4")]
    pub indent_amount: Option<i32>,
    #[prost(message, optional, tag = "5")]
    pub checklist: Option<Checklist>,
    #[prost(int32, optional, tag = "8")]
    pub block_quote: Option<i32>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct AttributeRun {
    #[prost(int32, required, tag = "1")]
    pub length: i32,
    #[prost(message, optional, tag = "2")]
    pub paragraph_style: Option<ParagraphStyle>,
    #[prost(message, optional, tag = "3")]
    pub font: Option<Font>,
    #[prost(int32, optional, tag = "5")]
    pub font_weight: Option<i32>,
    #[prost(int32, optional, tag = "6")]
    pub underlined: Option<i32>,
    #[prost(int32, optional, tag = "7")]
    pub strikethrough: Option<i32>,
    #[prost(int32, optional, tag = "8")]
    pub superscript: Option<i32>,
    #[prost(string, optional, tag = "9")]
    pub link: Option<String>,
    #[prost(message, optional, tag = "10")]
    pub color: Option<Color>,
    #[prost(message, optional, tag = "12")]
    pub attachment_info: Option<AttachmentInfo>,
    #[prost(int32, optional, tag = "13")]
    pub unknown_identifier: Option<i32>,
    #[prost(int32, optional, tag = "14")]
    pub emphasis_style: Option<i32>,
}

impl AttributeRun {
    /// Check if two AttributeRuns have the same style
    pub fn same_style(&self, other: &AttributeRun) -> bool {
        self.font_weight == other.font_weight
            && self.underlined == other.underlined
            && self.strikethrough == other.strikethrough
            && self.superscript == other.superscript
            && self.link == other.link
            && self.paragraph_style == other.paragraph_style
            && self.font == other.font
            && self.color == other.color
    }
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct Note {
    #[prost(string, required, tag = "2")]
    pub note_text: String,
    #[prost(message, repeated, tag = "5")]
    pub attribute_run: Vec<AttributeRun>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct Document {
    #[prost(int32, required, tag = "2")]
    pub version: i32,
    #[prost(message, required, tag = "3")]
    pub note: Note,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct NoteStoreProto {
    #[prost(message, required, tag = "2")]
    pub document: Document,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct ObjectId {
    #[prost(uint64, required, tag = "2")]
    pub unsigned_integer_value: u64,
    #[prost(string, required, tag = "4")]
    pub string_value: String,
    #[prost(int32, required, tag = "6")]
    pub object_index: i32,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct DictionaryElement {
    #[prost(message, required, tag = "1")]
    pub key: ObjectId,
    #[prost(message, required, tag = "2")]
    pub value: ObjectId,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct Dictionary {
    #[prost(message, repeated, tag = "1")]
    pub element: Vec<DictionaryElement>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct RegisterLatest {
    #[prost(message, required, tag = "2")]
    pub contents: ObjectId,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct MapEntry {
    #[prost(int32, required, tag = "1")]
    pub key: i32,
    #[prost(message, required, tag = "2")]
    pub value: ObjectId,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct MergeableDataObjectMap {
    #[prost(int32, required, tag = "1")]
    pub r#type: i32,
    #[prost(message, repeated, tag = "3")]
    pub map_entry: Vec<MapEntry>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct OrderedSetOrderingArrayAttachment {
    #[prost(int32, required, tag = "1")]
    pub index: i32,
    #[prost(bytes, required, tag = "2")]
    pub uuid: Vec<u8>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct OrderedSetOrderingArray {
    #[prost(message, required, tag = "1")]
    pub contents: Note,
    #[prost(message, repeated, tag = "2")]
    pub attachment: Vec<OrderedSetOrderingArrayAttachment>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct OrderedSetOrdering {
    #[prost(message, required, tag = "1")]
    pub array: OrderedSetOrderingArray,
    #[prost(message, required, tag = "2")]
    pub contents: Dictionary,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct OrderedSet {
    #[prost(message, required, tag = "1")]
    pub ordering: OrderedSetOrdering,
    #[prost(message, required, tag = "2")]
    pub elements: Dictionary,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct ListEntryDetailsKey {
    #[prost(int32, required, tag = "1")]
    pub list_entry_details_type_index: i32,
    #[prost(int32, required, tag = "2")]
    pub list_entry_details_key: i32,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct ListEntryDetails {
    #[prost(message, optional, tag = "1")]
    pub list_entry_details_key: Option<ListEntryDetailsKey>,
    #[prost(message, optional, tag = "2")]
    pub id: Option<ObjectId>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct ListEntry {
    #[prost(message, required, tag = "2")]
    pub id: ObjectId,
    #[prost(message, optional, tag = "3")]
    pub details: Option<ListEntryDetails>,
    #[prost(message, required, tag = "4")]
    pub additional_details: ListEntryDetails,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct List {
    #[prost(message, repeated, tag = "1")]
    pub list_entry: Vec<ListEntry>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct UnknownMergeableDataObjectEntryMessageEntry {
    #[prost(int32, optional, tag = "1")]
    pub unknown_int1: Option<i32>,
    #[prost(int64, optional, tag = "2")]
    pub unknown_int2: Option<i64>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct UnknownMergeableDataObjectEntryMessage {
    #[prost(message, optional, tag = "1")]
    pub unknown_entry: Option<UnknownMergeableDataObjectEntryMessageEntry>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct MergeableDataObjectEntry {
    #[prost(message, required, tag = "1")]
    pub register_latest: RegisterLatest,
    #[prost(message, optional, tag = "5")]
    pub list: Option<List>,
    #[prost(message, optional, tag = "6")]
    pub dictionary: Option<Dictionary>,
    #[prost(message, optional, tag = "9")]
    pub unknown_message: Option<UnknownMergeableDataObjectEntryMessage>,
    #[prost(message, optional, tag = "10")]
    pub note: Option<Note>,
    #[prost(message, optional, tag = "13")]
    pub custom_map: Option<MergeableDataObjectMap>,
    #[prost(message, optional, tag = "16")]
    pub ordered_set: Option<OrderedSet>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct MergeableDataObjectData {
    #[prost(message, repeated, tag = "3")]
    pub mergeable_data_object_entry: Vec<MergeableDataObjectEntry>,
    #[prost(string, repeated, tag = "4")]
    pub mergeable_data_object_key_item: Vec<String>,
    #[prost(string, repeated, tag = "5")]
    pub mergeable_data_object_type_item: Vec<String>,
    #[prost(bytes = "vec", repeated, tag = "6")]
    pub mergeable_data_object_uuid_item: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct MergeableDataObject {
    #[prost(int32, required, tag = "2")]
    pub version: i32,
    #[prost(message, required, tag = "3")]
    pub mergeable_data_object_data: MergeableDataObjectData,
}

#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct MergableDataProto {
    #[prost(message, required, tag = "2")]
    pub mergable_data_object: MergeableDataObject,
}
