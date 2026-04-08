use std::path::Path;

use url::Url;

#[derive(Debug, Clone, Default)]
pub struct NewThreadDeepLinkOptions<'a> {
    pub path: Option<&'a Path>,
    pub origin_url: Option<&'a str>,
    pub prompt: Option<&'a str>,
}

pub fn thread_deeplink(thread_id: &str) -> String {
    format!("codex://threads/{thread_id}")
}

pub fn new_thread_deeplink(options: NewThreadDeepLinkOptions<'_>) -> String {
    let mut url = Url::parse("codex://new").expect("valid codex deeplink base");
    let has_query =
        options.path.is_some() || options.origin_url.is_some() || options.prompt.is_some();

    if !has_query {
        return url.to_string().trim_end_matches('?').to_string();
    }

    {
        let mut query = url.query_pairs_mut();

        if let Some(path) = options.path {
            query.append_pair("path", &path.display().to_string());
        }

        if let Some(origin_url) = options.origin_url {
            query.append_pair("originUrl", origin_url);
        }

        if let Some(prompt) = options.prompt {
            query.append_pair("prompt", prompt);
        }
    }

    url.to_string()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{NewThreadDeepLinkOptions, new_thread_deeplink, thread_deeplink};

    #[test]
    fn builds_thread_deeplink() {
        assert_eq!(
            thread_deeplink("123e4567-e89b-12d3-a456-426614174000"),
            "codex://threads/123e4567-e89b-12d3-a456-426614174000"
        );
    }

    #[test]
    fn builds_new_thread_deeplink_with_all_options() {
        let deeplink = new_thread_deeplink(NewThreadDeepLinkOptions {
            path: Some(Path::new("/Users/test/project")),
            origin_url: Some("https://github.com/openai/codex.git"),
            prompt: Some("Fix flaky test"),
        });

        assert_eq!(
            deeplink,
            "codex://new?path=%2FUsers%2Ftest%2Fproject&originUrl=https%3A%2F%2Fgithub.com%2Fopenai%2Fcodex.git&prompt=Fix+flaky+test"
        );
    }

    #[test]
    fn skips_absent_new_thread_options() {
        assert_eq!(
            new_thread_deeplink(NewThreadDeepLinkOptions::default()),
            "codex://new"
        );
    }
}
