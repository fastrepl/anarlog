use hypr_http::HttpClient;

use crate::error::Error;
use crate::types::{Issue, IssueComment, ListIssuesRequest, ListReposRequest, Repository};

pub struct GitHubIssuesClient<C> {
    http: C,
}

impl<C: HttpClient> GitHubIssuesClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_repos(&self, req: ListReposRequest) -> Result<Vec<Repository>, Error> {
        let mut query_parts: Vec<String> = Vec::new();

        if let Some(per_page) = req.per_page {
            query_parts.push(format!("per_page={per_page}"));
        }
        if let Some(page) = req.page {
            query_parts.push(format!("page={page}"));
        }
        if let Some(ref sort) = req.sort {
            query_parts.push(format!("sort={}", urlencoding::encode(sort)));
        }

        let path = if query_parts.is_empty() {
            "/user/repos".to_string()
        } else {
            format!("/user/repos?{}", query_parts.join("&"))
        };

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let repos: Vec<Repository> = serde_json::from_slice(&bytes)?;
        Ok(repos)
    }

    pub async fn get_issue(&self, owner: &str, repo: &str, number: u64) -> Result<Issue, Error> {
        let path = format!("/repos/{owner}/{repo}/issues/{number}");
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let issue: Issue = serde_json::from_slice(&bytes)?;
        Ok(issue)
    }

    pub async fn list_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<IssueComment>, Error> {
        let path = format!("/repos/{owner}/{repo}/issues/{number}/comments?per_page=100");
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let comments: Vec<IssueComment> = serde_json::from_slice(&bytes)?;
        Ok(comments)
    }

    pub async fn list_issues(&self, req: ListIssuesRequest) -> Result<Vec<Issue>, Error> {
        let mut query_parts: Vec<String> = Vec::new();

        if let Some(ref state) = req.state {
            query_parts.push(format!("state={}", state.as_str()));
        }
        if let Some(ref labels) = req.labels
            && !labels.is_empty()
        {
            let joined = labels
                .iter()
                .map(|l| urlencoding::encode(l).into_owned())
                .collect::<Vec<_>>()
                .join(",");
            query_parts.push(format!("labels={joined}"));
        }
        if let Some(ref assignee) = req.assignee {
            query_parts.push(format!("assignee={}", urlencoding::encode(assignee)));
        }
        if let Some(ref sort) = req.sort {
            query_parts.push(format!("sort={}", urlencoding::encode(sort)));
        }
        if let Some(ref direction) = req.direction {
            query_parts.push(format!("direction={}", urlencoding::encode(direction)));
        }
        if let Some(per_page) = req.per_page {
            query_parts.push(format!("per_page={per_page}"));
        }
        if let Some(page) = req.page {
            query_parts.push(format!("page={page}"));
        }

        let path = if query_parts.is_empty() {
            format!("/repos/{}/{}/issues", req.owner, req.repo)
        } else {
            format!(
                "/repos/{}/{}/issues?{}",
                req.owner,
                req.repo,
                query_parts.join("&")
            )
        };

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let issues: Vec<Issue> = serde_json::from_slice(&bytes)?;
        Ok(issues)
    }
}
