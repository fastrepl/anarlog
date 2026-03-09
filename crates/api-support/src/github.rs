use crate::error::{Result, SupportError};
use crate::logs;
use crate::state::AppState;

const GITHUB_OWNER: &str = "fastrepl";
const GITHUB_REPO: &str = "char";

pub(crate) struct BugReportInput<'a> {
    pub description: &'a str,
    pub platform: &'a str,
    pub arch: &'a str,
    pub os_version: &'a str,
    pub app_version: &'a str,
    pub source: &'a str,
    pub logs: Option<&'a str>,
}

pub(crate) struct FeatureRequestInput<'a> {
    pub description: &'a str,
    pub platform: &'a str,
    pub arch: &'a str,
    pub os_version: &'a str,
    pub app_version: &'a str,
    pub source: &'a str,
}

pub(crate) async fn submit_bug_report(
    state: &AppState,
    input: BugReportInput<'_>,
) -> Result<String> {
    let (description, title) = make_title(input.description, "Bug Report");

    let body = hypr_template_support::render(hypr_template_support::SupportTemplate::BugReport(
        hypr_template_support::BugReport {
            description,
            platform: input.platform.to_string(),
            arch: input.arch.to_string(),
            os_version: input.os_version.to_string(),
            app_version: input.app_version.to_string(),
            source: input.source.to_string(),
        },
    ))
    .map_err(|e| SupportError::Internal(e.to_string()))?;

    let labels = vec!["product/desktop".to_string(), "Engineering".to_string()];
    let (url, number) = create_issue(state, &title, &body, &labels, Some("Bug")).await?;

    if let Some(logs) = input.logs {
        attach_log_analysis(state, number, logs).await;
    }

    Ok(url)
}

pub(crate) async fn submit_feature_request(
    state: &AppState,
    input: FeatureRequestInput<'_>,
) -> Result<String> {
    let (description, title) = make_title(input.description, "Feature Request");

    let body =
        hypr_template_support::render(hypr_template_support::SupportTemplate::FeatureRequest(
            hypr_template_support::FeatureRequest {
                description,
                platform: input.platform.to_string(),
                arch: input.arch.to_string(),
                os_version: input.os_version.to_string(),
                app_version: input.app_version.to_string(),
                source: input.source.to_string(),
            },
        ))
        .map_err(|e| SupportError::Internal(e.to_string()))?;

    let category_id = &state.config.github.github_discussion_category_id;
    if category_id.is_empty() {
        return Err(SupportError::Internal(
            "GitHub discussion category not configured".to_string(),
        ));
    }

    create_discussion(state, &title, &body, category_id).await
}

fn make_title(description: &str, fallback: &str) -> (String, String) {
    let description = description.trim().to_string();
    let first_line = description
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(100)
        .collect::<String>();
    let title = if first_line.is_empty() {
        fallback.to_string()
    } else {
        first_line
    };
    (description, title)
}

async fn attach_log_analysis(state: &AppState, issue_number: u64, log_text: &str) {
    let clean_logs = logs::strip_ansi_escapes(log_text);

    let log_summary =
        logs::analyze_logs(&state.config.openrouter.openrouter_api_key, &clean_logs).await;

    let summary_section = match log_summary.as_deref() {
        Some(s) if !s.trim().is_empty() => format!("### Summary\n```\n{s}\n```"),
        _ => "_No errors or warnings found._".to_string(),
    };

    let tail = logs::safe_tail(&clean_logs, 10000);
    let log_comment = hypr_template_support::render(
        hypr_template_support::SupportTemplate::LogAnalysis(hypr_template_support::LogAnalysis {
            summary_section,
            tail: tail.to_string(),
        }),
    )
    .unwrap_or_default();

    let _ = add_issue_comment(state, issue_number, &log_comment).await;
}

pub(crate) async fn create_issue(
    state: &AppState,
    title: &str,
    body: &str,
    labels: &[String],
    issue_type: Option<&str>,
) -> Result<(String, u64)> {
    let client = state.installation_client().await?;

    let mut payload = serde_json::json!({
        "title": title,
        "body": body,
        "labels": labels,
    });

    if let Some(t) = issue_type {
        payload["type"] = serde_json::Value::String(t.to_string());
    }

    let issue: octocrab::models::issues::Issue = client
        .post(
            format!("/repos/{GITHUB_OWNER}/{GITHUB_REPO}/issues"),
            Some(&payload),
        )
        .await?;

    Ok((issue.html_url.to_string(), issue.number))
}

pub(crate) async fn add_issue_comment(
    state: &AppState,
    issue_number: u64,
    comment: &str,
) -> Result<String> {
    let client = state.installation_client().await?;
    let comment = client
        .issues(GITHUB_OWNER, GITHUB_REPO)
        .create_comment(issue_number, comment)
        .await?;
    Ok(comment.html_url.to_string())
}

pub(crate) async fn search_issues(
    state: &AppState,
    query: &str,
    state_filter: Option<&str>,
    limit: u8,
) -> Result<Vec<serde_json::Value>> {
    let client = state.installation_client().await?;

    let mut q = format!("repo:{GITHUB_OWNER}/{GITHUB_REPO} is:issue {query}");
    if let Some(s) = state_filter {
        match s {
            "open" | "closed" => q.push_str(&format!(" is:{s}")),
            _ => {
                return Err(SupportError::Internal(
                    "Invalid state filter: must be 'open' or 'closed'".to_string(),
                ));
            }
        }
    }

    let result = client
        .search()
        .issues_and_pull_requests(&q)
        .per_page(limit)
        .send()
        .await?;

    let items: Vec<serde_json::Value> = result
        .items
        .into_iter()
        .map(|issue| {
            serde_json::json!({
                "number": issue.number,
                "title": issue.title,
                "state": format!("{:?}", issue.state).to_lowercase(),
                "url": issue.html_url.to_string(),
                "created_at": issue.created_at.to_rfc3339(),
                "labels": issue.labels.iter().map(|l| &l.name).collect::<Vec<_>>(),
            })
        })
        .collect();

    Ok(items)
}

async fn create_discussion(
    state: &AppState,
    title: &str,
    body: &str,
    category_id: &str,
) -> Result<String> {
    let client = state.installation_client().await?;

    let query = serde_json::json!({
        "query": r#"
            mutation($repositoryId: ID!, $categoryId: ID!, $title: String!, $body: String!) {
                createDiscussion(input: {
                    repositoryId: $repositoryId
                    categoryId: $categoryId
                    title: $title
                    body: $body
                }) {
                    discussion {
                        url
                    }
                }
            }
        "#,
        "variables": {
            "repositoryId": state.config.github.github_repo_id,
            "categoryId": category_id,
            "title": title,
            "body": body,
        },
    });

    let data: serde_json::Value = client.graphql(&query).await?;

    data["data"]["createDiscussion"]["discussion"]["url"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            SupportError::GitHub(format!(
                "unexpected GraphQL response: {}",
                serde_json::to_string(&data).unwrap_or_default()
            ))
        })
}
