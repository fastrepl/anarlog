use crate::user_common_derives;

user_common_derives! {
    #[sql_table("extension_mappings")]
    pub struct ExtensionMapping {
        pub id: String,
        pub user_id: String,
        pub extension_id: String,
        pub config: serde_json::Value,
        pub widgets: Vec<ExtensionWidget>,
    }
}

user_common_derives! {
    pub struct ExtensionWidget {
        pub kind: ExtensionWidgetKind,
        pub group: String,
        pub position: Option<ExtensionWidgetPosition>,
    }
}

user_common_derives! {
    pub enum ExtensionWidgetKind {
        #[serde(rename = "oneByOne")]
        OneByOne,
        #[serde(rename = "twoByOne")]
        TwoByOne,
        #[serde(rename = "twoByTwo")]
        TwoByTwo,
        #[serde(rename = "full")]
        Full,
    }
}

user_common_derives! {
    pub struct ExtensionWidgetPosition {
        pub x: u8,
        pub y: u8,
    }
}

impl ExtensionMapping {
    pub fn from_row(row: &libsql::Row) -> Result<Self, serde::de::value::Error> {
        Ok(Self {
            id: row.get(0).expect("id"),
            extension_id: row.get(1).expect("extension_id"),
            user_id: row.get(2).expect("user_id"),
            config: row
                .get_str(3)
                .map(|s| serde_json::from_str(s).unwrap())
                .unwrap_or_default(),
            widgets: row
                .get_str(4)
                .map(|s| serde_json::from_str(s).unwrap())
                .unwrap_or_default(),
        })
    }
}

user_common_derives! {
    pub struct ExtensionDefinition {
        pub id: String,
        pub title: String,
        pub description: String,
        pub implemented: bool,
        pub default: bool,
        pub cloud_only: bool,
        pub plugins: Vec<String>,
        pub tags: Vec<String>,
    }
}
