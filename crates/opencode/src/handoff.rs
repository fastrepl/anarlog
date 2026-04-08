use std::path::Path;

use url::Url;

pub fn open_project_deeplink(directory: &Path) -> String {
    let mut url = Url::parse("opencode://open-project").expect("valid opencode deeplink base");
    url.query_pairs_mut()
        .append_pair("directory", &directory.display().to_string());
    url.to_string()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::open_project_deeplink;

    #[test]
    fn builds_open_project_deeplink() {
        assert_eq!(
            open_project_deeplink(Path::new("/Users/test/project")),
            "opencode://open-project?directory=%2FUsers%2Ftest%2Fproject"
        );
    }
}
