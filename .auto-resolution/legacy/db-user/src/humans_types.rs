use crate::user_common_derives;

user_common_derives! {
    #[sql_table("humans")]
    pub struct Human {
        pub id: String,
        pub organization_id: Option<String>,
        pub is_user: bool,
        pub full_name: Option<String>,
        pub email: Option<String>,
        pub job_title: Option<String>,
        pub linkedin_username: Option<String>,
    }
}

user_common_derives! {
    pub enum ListHumanFilter {
        #[serde(rename = "search")]
        Search((u8, String)),
    }
}

impl Default for Human {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            organization_id: None,
            is_user: false,
            full_name: None,
            email: None,
            job_title: None,
            linkedin_username: None,
        }
    }
}
