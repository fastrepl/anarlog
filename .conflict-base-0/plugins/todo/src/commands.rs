use hypr_apple_todo::types::{
    CreateReminderInput, Reminder, ReminderFilter, ReminderIdentifierInput, ReminderList,
};
use hypr_ticket_interface::{CollectionPage, TicketPage};

use tauri::Manager;
use tauri_plugin_auth::AuthPluginExt;

use crate::error::Error;
use crate::read_path::{ReadPath, ReadPathResult};

#[tauri::command]
#[specta::specta]
pub fn authorization_status() -> Result<String, Error> {
    #[cfg(target_os = "macos")]
    {
        let status = hypr_apple_todo::Handle::authorization_status();
        Ok(format!("{:?}", status))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn request_full_access() -> Result<bool, Error> {
    #[cfg(target_os = "macos")]
    {
        Ok(hypr_apple_todo::Handle::request_full_access())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn list_todo_lists() -> Result<Vec<ReminderList>, Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.list_reminder_lists().map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn fetch_todos(filter: ReminderFilter) -> Result<Vec<Reminder>, Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.fetch_reminders(filter).map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = filter;
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub async fn read_path<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
    limit: Option<u32>,
    cursor: Option<String>,
) -> Result<ReadPathResult, Error> {
    match ReadPath::parse(&path)? {
        ReadPath::Apple(path) => {
            #[cfg(target_os = "macos")]
            {
                let handle = hypr_apple_todo::Handle;
                match handle.read_path(path)? {
                    hypr_apple_todo::ReadPathResult::Lists(items) => {
                        Ok(ReadPathResult::ReminderLists(items))
                    }
                    hypr_apple_todo::ReadPathResult::Reminders(items) => {
                        Ok(ReadPathResult::Reminders(items))
                    }
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                Err(Error::UnsupportedPlatform)
            }
        }
        ReadPath::LinearTeams { connection_id } => {
            let config = app.state::<crate::PluginConfig>();
            let token = require_access_token(&app)?;
            crate::fetch::linear_list_teams(
                &config.api_base_url,
                &token,
                connection_id,
                limit,
                cursor,
            )
            .await
            .map(ReadPathResult::Collections)
        }
        ReadPath::LinearTickets {
            connection_id,
            team_id,
        } => {
            let config = app.state::<crate::PluginConfig>();
            let token = require_access_token(&app)?;
            crate::fetch::linear_list_tickets(
                &config.api_base_url,
                &token,
                connection_id,
                team_id,
                None,
                limit,
                cursor,
            )
            .await
            .map(ReadPathResult::Tickets)
        }
        ReadPath::GithubRepos { connection_id } => {
            let config = app.state::<crate::PluginConfig>();
            let token = require_access_token(&app)?;
            crate::fetch::github_list_repos(
                &config.api_base_url,
                &token,
                connection_id,
                limit,
                cursor,
            )
            .await
            .map(ReadPathResult::Collections)
        }
        ReadPath::GithubTickets {
            connection_id,
            owner,
            repo,
        } => {
            let config = app.state::<crate::PluginConfig>();
            let token = require_access_token(&app)?;
            crate::fetch::github_list_tickets(
                &config.api_base_url,
                &token,
                connection_id,
                owner,
                repo,
                limit,
                cursor,
            )
            .await
            .map(ReadPathResult::Tickets)
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn create_todo(input: CreateReminderInput) -> Result<String, Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.create_reminder_identifier(input).map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = input;
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn complete_todo(target: ReminderIdentifierInput) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.complete_reminder(&target).map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = target;
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub fn delete_todo(target: ReminderIdentifierInput) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_todo::Handle;
        handle.delete_reminder(&target).map_err(Into::into)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = target;
        Err(Error::UnsupportedPlatform)
    }
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_teams<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    connection_id: String,
    limit: Option<u32>,
    cursor: Option<String>,
) -> Result<CollectionPage, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = require_access_token(&app)?;
    crate::fetch::linear_list_teams(&config.api_base_url, &token, &connection_id, limit, cursor)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_tickets<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    connection_id: String,
    team_id: String,
    query: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
) -> Result<TicketPage, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = require_access_token(&app)?;
    crate::fetch::linear_list_tickets(
        &config.api_base_url,
        &token,
        &connection_id,
        &team_id,
        query,
        limit,
        cursor,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn github_issue_state(
    owner: String,
    repo: String,
    number: u64,
) -> Result<crate::github_state::GitHubIssueState, Error> {
    crate::github_state::fetch_public(&owner, &repo, number).await
}

#[tauri::command]
#[specta::specta]
pub async fn github_issue_detail(
    owner: String,
    repo: String,
    number: u64,
) -> Result<hypr_github_issues::Issue, Error> {
    crate::github_state::fetch_issue_detail(&owner, &repo, number).await
}

#[tauri::command]
#[specta::specta]
pub async fn github_issue_comments(
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<hypr_github_issues::IssueComment>, Error> {
    crate::github_state::fetch_issue_comments(&owner, &repo, number).await
}

fn require_access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, Error> {
    let token = app.access_token().map_err(|e| Error::Auth(e.to_string()))?;
    match token {
        Some(t) if !t.is_empty() => Ok(t),
        _ => Err(Error::Auth("not authenticated".to_string())),
    }
}
