use crate::user_common_derives;

user_common_derives! {
    #[sql_table("organizations")]
    pub struct Organization {
        pub id: String,
        pub name: String,
        pub description: Option<String>,
    }
}

user_common_derives! {
    pub enum ListOrganizationFilter {
        #[serde(rename = "search")]
        Search((u8, String)),
    }
}
