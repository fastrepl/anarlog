use hypr_github_issues::{GitHubIssuesClient, Issue, IssueComment};

use crate::error::Error;

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub enum GitHubIssueState {
    Open,
    Closed,
    Merged,
}

struct PublicGitHubHttpClient {
    client: reqwest::Client,
}

impl PublicGitHubHttpClient {
    fn new() -> Result<Self, Error> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static("hypr-desktop"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self { client })
    }
}

fn unsupported_request(method: &str) -> Result<Vec<u8>, hypr_http::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("public GitHub client does not support {method} requests"),
    )
    .into())
}

impl hypr_http::HttpClient for PublicGitHubHttpClient {
    async fn get(&self, path: &str) -> Result<Vec<u8>, hypr_http::Error> {
        let url = format!("https://api.github.com{path}");
        let resp = self.client.get(&url).send().await.map_err(Box::new)?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(Box::new)?;
        if !status.is_success() {
            return Err(format!("GitHub API returned {status}").into());
        }
        Ok(bytes.to_vec())
    }

    async fn post(
        &self,
        _path: &str,
        _body: Vec<u8>,
        _content_type: &str,
    ) -> Result<Vec<u8>, hypr_http::Error> {
        unsupported_request("POST")
    }

    async fn put(&self, _path: &str, _body: Vec<u8>) -> Result<Vec<u8>, hypr_http::Error> {
        unsupported_request("PUT")
    }

    async fn patch(&self, _path: &str, _body: Vec<u8>) -> Result<Vec<u8>, hypr_http::Error> {
        unsupported_request("PATCH")
    }

    async fn delete(&self, _path: &str) -> Result<Vec<u8>, hypr_http::Error> {
        unsupported_request("DELETE")
    }
}

fn resolve_state(issue: &Issue) -> GitHubIssueState {
    if let Some(ref pr) = issue.pull_request {
        if pr.merged_at.is_some() {
            return GitHubIssueState::Merged;
        }
    }

    match issue.state.as_str() {
        "closed" => GitHubIssueState::Closed,
        _ => GitHubIssueState::Open,
    }
}

/// Fetch issue/PR state from the public GitHub API (no auth).
/// Returns an error for private repos — caller should fall back to proxy.
pub async fn fetch_public(owner: &str, repo: &str, number: u64) -> Result<GitHubIssueState, Error> {
    let http = PublicGitHubHttpClient::new()?;
    let client = GitHubIssuesClient::new(http);
    let issue = client
        .get_issue(owner, repo, number)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;
    Ok(resolve_state(&issue))
}

/// Fetch the full issue/PR detail from the public GitHub API (no auth).
pub async fn fetch_issue_detail(owner: &str, repo: &str, number: u64) -> Result<Issue, Error> {
    let http = PublicGitHubHttpClient::new()?;
    let client = GitHubIssuesClient::new(http);
    let issue = client
        .get_issue(owner, repo, number)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;
    Ok(issue)
}

/// Fetch comments for an issue/PR from the public GitHub API (no auth).
pub async fn fetch_issue_comments(
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<Vec<IssueComment>, Error> {
    let http = PublicGitHubHttpClient::new()?;
    let client = GitHubIssuesClient::new(http);
    let comments = client
        .list_comments(owner, repo, number)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;
    Ok(comments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_requests_return_recoverable_errors() {
        for method in ["POST", "PUT", "PATCH", "DELETE"] {
            let error = unsupported_request(method).unwrap_err();

            assert_eq!(
                error.to_string(),
                format!("public GitHub client does not support {method} requests")
            );
        }
    }
}
