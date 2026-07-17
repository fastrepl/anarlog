use crate::common_derives;

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ReaderResponseEnvelope {
        pub code: u16,
        pub status: u32,
        pub data: ReaderData,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ReaderData {
        #[serde(default)]
        pub title: String,
        #[serde(default)]
        pub url: String,
        #[serde(default)]
        pub content: String,
        #[serde(default)]
        pub description: String,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct SearchResponseEnvelope {
        pub code: u16,
        pub status: u32,
        pub data: Vec<SearchResultItem>,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct SearchResultItem {
        #[serde(default)]
        pub title: String,
        #[serde(default)]
        pub url: String,
        #[serde(default)]
        pub content: String,
        #[serde(default)]
        pub description: String,
    }
}

common_derives! {
    pub enum RespondWith {
        #[serde(rename = "markdown")]
        Markdown,
        #[serde(rename = "html")]
        Html,
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "screenshot")]
        Screenshot,
        #[serde(rename = "pageshot")]
        Pageshot,
    }
}

common_derives! {
    pub enum RetainImages {
        #[serde(rename = "none")]
        None,
        #[serde(rename = "all")]
        All,
        #[serde(rename = "alt")]
        Alt,
    }
}

common_derives! {
    pub enum SearchType {
        #[serde(rename = "web")]
        Web,
        #[serde(rename = "images")]
        Images,
        #[serde(rename = "news")]
        News,
    }
}

common_derives! {
    pub enum SearchEngine {
        #[serde(rename = "google")]
        Google,
        #[serde(rename = "bing")]
        Bing,
    }
}
