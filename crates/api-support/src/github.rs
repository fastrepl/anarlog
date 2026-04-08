use crate::error::{Result, SupportError};
use crate::logs;
use crate::state::AppState;
use serde::Deserialize;
use std::collections::HashMap;

const GITHUB_OWNER: &str = "fastrepl";
const GITHUB_REPO: &str = "char";
const DEFAULT_ISSUE_LABELS: &[&str] = &[
    "area/backend",
    "area/ui",
    "Engineering",
    "os/linux",
    "os/macos",
    "os/windows",
    "product/cli",
    "product/desktop",
    "product/owhisper",
    "product/slack-bot",
    "product/web",
];

#[derive(Debug, Deserialize)]
struct GitHubLabel {
    name: String,
}

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

    let labels = resolve_issue_labels(
        state,
        &title,
        &body,
        &[
            "product/desktop".to_string(),
            platform_to_label(input.platform).to_string(),
        ],
    )
    .await?;
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

pub(crate) async fn get_repo_labels(state: &AppState) -> Result<Vec<String>> {
    let client = state.installation_client().await?;
    let labels: Vec<GitHubLabel> = client
        .get(
            format!("/repos/{GITHUB_OWNER}/{GITHUB_REPO}/labels?per_page=100"),
            None::<&()>,
        )
        .await?;
    Ok(labels.into_iter().map(|label| label.name).collect())
}

pub(crate) async fn resolve_issue_labels(
    state: &AppState,
    title: &str,
    body: &str,
    requested_labels: &[String],
) -> Result<Vec<String>> {
    let repo_labels = match get_repo_labels(state).await {
        Ok(labels) => labels,
        Err(error) => {
            tracing::warn!(error = %error, "failed_to_fetch_repo_labels");
            fallback_repo_labels()
        }
    };

    Ok(select_issue_labels(
        &repo_labels,
        title,
        body,
        requested_labels,
    ))
}

fn fallback_repo_labels() -> Vec<String> {
    DEFAULT_ISSUE_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect()
}

fn platform_to_label(platform: &str) -> &'static str {
    let platform = platform.trim().to_ascii_lowercase();
    if platform.contains("mac") {
        "os/macos"
    } else if platform.contains("win") {
        "os/windows"
    } else if platform.contains("linux") {
        "os/linux"
    } else {
        ""
    }
}

fn select_issue_labels(
    repo_labels: &[String],
    title: &str,
    body: &str,
    requested_labels: &[String],
) -> Vec<String> {
    let repo_labels_by_key: HashMap<String, &str> = repo_labels
        .iter()
        .map(|label| (normalize_label(label), label.as_str()))
        .collect();
    let mut selected = Vec::new();

    let mut push_label = |label: &str| {
        let normalized = normalize_label(label);
        if let Some(existing) = repo_labels_by_key.get(&normalized) {
            let existing = existing.to_string();
            if !selected.contains(&existing) {
                selected.push(existing);
            }
        }
    };

    for label in requested_labels {
        push_label(label);
    }

    let text = normalize_search_text(&format!("{title}\n{body}"));

    for (needle, label) in [
        ("desktop", "product/desktop"),
        ("cli", "product/cli"),
        ("command line", "product/cli"),
        ("terminal", "product/cli"),
        ("web", "product/web"),
        ("browser", "product/web"),
        ("slack", "product/slack-bot"),
        ("owhisper", "product/owhisper"),
        ("macos", "os/macos"),
        ("mac os", "os/macos"),
        ("darwin", "os/macos"),
        ("windows", "os/windows"),
        ("linux", "os/linux"),
    ] {
        if contains_term(&text, needle) {
            push_label(label);
        }
    }

    if contains_any(
        &text,
        &[
            "ui", "ux", "button", "dialog", "modal", "screen", "layout", "render", "sidebar",
            "editor",
        ],
    ) {
        push_label("area/ui");
    }

    if contains_any(
        &text,
        &[
            "api", "backend", "server", "database", "db", "auth", "sync", "webhook", "mcp",
        ],
    ) {
        push_label("area/backend");
    }

    push_label("Engineering");

    selected
}

fn normalize_label(label: &str) -> String {
    label.trim().to_ascii_lowercase()
}

fn normalize_search_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len() + 2);
    normalized.push(' ');

    let mut last_was_space = true;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    if !last_was_space {
        normalized.push(' ');
    }

    normalized
}

fn contains_term(text: &str, needle: &str) -> bool {
    let needle = normalize_search_text(needle);
    text.contains(&needle)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| contains_term(text, needle))
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

#[cfg(test)]
mod tests {
    use super::{fallback_repo_labels, select_issue_labels};

    fn repo_labels() -> Vec<String> {
        vec![
            "area/backend".to_string(),
            "area/ui".to_string(),
            "Engineering".to_string(),
            "os/linux".to_string(),
            "os/macos".to_string(),
            "os/windows".to_string(),
            "product/cli".to_string(),
            "product/desktop".to_string(),
            "product/owhisper".to_string(),
            "product/slack-bot".to_string(),
            "product/web".to_string(),
        ]
    }

    #[test]
    fn selects_requested_and_inferred_labels() {
        let labels = select_issue_labels(
            &repo_labels(),
            "Desktop UI crash on macOS",
            "The desktop app crashes when opening the sidebar on macOS.",
            &["product/desktop".to_string()],
        );

        assert_eq!(
            labels,
            vec![
                "product/desktop".to_string(),
                "os/macos".to_string(),
                "area/ui".to_string(),
                "Engineering".to_string(),
            ]
        );
    }

    #[test]
    fn ignores_unknown_labels_and_keeps_known_matches() {
        let labels = select_issue_labels(
            &repo_labels(),
            "API sync fails on Linux",
            "The backend webhook handler returns 500 on Linux.",
            &["unknown".to_string(), "OS/Linux".to_string()],
        );

        assert_eq!(
            labels,
            vec![
                "os/linux".to_string(),
                "area/backend".to_string(),
                "Engineering".to_string(),
            ]
        );
    }

    #[test]
    fn fallback_labels_preserve_safe_defaults() {
        let labels = select_issue_labels(
            &fallback_repo_labels(),
            "Desktop UI crash on macOS",
            "The desktop app crashes when opening the sidebar on macOS.",
            &["product/desktop".to_string()],
        );

        assert_eq!(
            labels,
            vec![
                "product/desktop".to_string(),
                "os/macos".to_string(),
                "area/ui".to_string(),
                "Engineering".to_string(),
            ]
        );
    }
}
