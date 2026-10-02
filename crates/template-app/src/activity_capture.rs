use crate::common_derives;
use anlg_askama_utils::filters;

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "activity-capture.system.md.jinja")]
    pub struct ActivityCaptureSystem {
        pub language: Option<String>,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "activity-capture.user.md.jinja")]
    pub struct ActivityCaptureUser {
        pub app_name: String,
        pub window_title: Option<String>,
        pub reason: String,
        pub fingerprint: String,
    }
}
