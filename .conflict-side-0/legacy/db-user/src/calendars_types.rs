use crate::user_common_derives;

user_common_derives! {
    pub struct Calendar {
        pub id: String,
        pub tracking_id: String,
        pub user_id: String,
        pub platform: Platform,
        pub name: String,
        pub selected: bool,
        pub source: Option<String>,
    }
}

user_common_derives! {
    #[derive(strum::Display)]
    pub enum Platform {
        #[strum(serialize = "Apple")]
        Apple,
        #[strum(serialize = "Google")]
        Google,
        #[strum(serialize = "Outlook")]
        Outlook,
    }
}
