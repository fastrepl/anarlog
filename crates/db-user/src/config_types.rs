use crate::user_common_derives;

user_common_derives! {
    pub struct Config {
        pub id: String,
        pub user_id: String,
        pub general: ConfigGeneral,
        pub notification: ConfigNotification,
        pub ai: ConfigAI,
    }
}

impl Config {
    pub fn from_row(row: &libsql::Row) -> Result<Self, serde::de::value::Error> {
        Ok(Self {
            id: row.get(0).expect("id"),
            user_id: row.get(1).expect("user_id"),
            general: row
                .get_str(2)
                .map(|s| serde_json::from_str(s).unwrap())
                .unwrap_or_default(),
            notification: row
                .get_str(3)
                .map(|s| serde_json::from_str(s).unwrap())
                .unwrap_or_default(),
            ai: row
                .get_str(4)
                .map(|s| serde_json::from_str(s).unwrap())
                .unwrap_or_default(),
        })
    }
}

user_common_derives! {
    pub struct ConfigGeneral {
        pub autostart: bool,
        #[specta(type = String)]
        #[schemars(with = "String", regex(pattern = "^[a-zA-Z]{2}$"))]
        pub display_language: hypr_language::Language,
        #[specta(type = Vec<String>)]
        #[serde(default)]
        pub spoken_languages: Vec<hypr_language::Language>,
        #[serde(default)]
        pub jargons: Vec<String>,
        pub telemetry_consent: bool,
        pub save_recordings: Option<bool>,
        pub selected_template_id: Option<String>,
        #[specta(type = String)]
        #[schemars(with = "String", regex(pattern = "^[a-zA-Z]{2}$"))]
        #[serde(default)]
        pub summary_language: hypr_language::Language,
    }
}

impl Default for ConfigGeneral {
    fn default() -> Self {
        Self {
            autostart: false,
            display_language: hypr_language::ISO639::En.into(),
            spoken_languages: vec![hypr_language::ISO639::En.into()],
            jargons: vec![],
            telemetry_consent: true,
            save_recordings: Some(false),
            selected_template_id: None,
            summary_language: hypr_language::ISO639::En.into(),
        }
    }
}

user_common_derives! {
    pub struct ConfigNotification {
        pub before: bool,
        pub auto: bool,
        #[serde(rename = "ignoredPlatforms")]
        pub ignored_platforms: Option<Vec<String>>,
        #[serde(rename = "includedPlatforms")]
        pub included_platforms: Option<Vec<String>>,
    }
}

impl Default for ConfigNotification {
    fn default() -> Self {
        Self {
            before: true,
            auto: true,
            ignored_platforms: None,
            included_platforms: None,
        }
    }
}

user_common_derives! {
    pub struct ConfigAI {
        pub api_base: Option<String>,
        pub api_key: Option<String>,
        pub ai_specificity: Option<u8>,
        pub redemption_time_ms: Option<u32>,
    }
}

impl Default for ConfigAI {
    fn default() -> Self {
        Self {
            api_base: None,
            api_key: None,
            ai_specificity: Some(3),
            redemption_time_ms: Some(500),
        }
    }
}
