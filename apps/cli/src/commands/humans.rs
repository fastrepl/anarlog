use clap::Subcommand;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use sqlx::SqlitePool;

use crate::error::{CliError, CliResult};
use crate::widgets::InlineBox;

#[derive(Subcommand)]
pub enum Commands {
    /// List all humans
    List,
    /// Add a new human
    Add {
        name: String,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        org: Option<String>,
        #[arg(long)]
        title: Option<String>,
    },
    /// Edit a human
    Edit {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        org: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        memo: Option<String>,
    },
    /// Show details for a human
    Show {
        #[arg(long)]
        id: String,
    },
    /// Remove a human
    Rm {
        #[arg(long)]
        id: String,
    },
}

pub async fn run(pool: &SqlitePool, command: Option<Commands>) -> CliResult<()> {
    match command {
        Some(Commands::Add {
            name,
            email,
            org,
            title,
        }) => {
            add(
                pool,
                &name,
                email.as_deref(),
                org.as_deref(),
                title.as_deref(),
            )
            .await
        }
        Some(Commands::Edit {
            id,
            name,
            email,
            org,
            title,
            memo,
        }) => {
            edit(
                pool,
                &id,
                name.as_deref(),
                email.as_deref(),
                org.as_deref(),
                title.as_deref(),
                memo.as_deref(),
            )
            .await
        }
        Some(Commands::Show { id }) => show(pool, &id).await,
        Some(Commands::Rm { id }) => rm(pool, &id).await,
        Some(Commands::List) | None => list(pool).await,
    }
}

async fn list(pool: &SqlitePool) -> CliResult<()> {
    let dim = Style::default().add_modifier(Modifier::DIM);

    let humans = hypr_db_app::list_humans(pool)
        .await
        .map_err(|e| CliError::operation_failed("list humans", e.to_string()))?;

    if humans.is_empty() {
        let lines = vec![Line::from(Span::styled("No humans found.", dim))];
        return render_info(lines);
    }

    let lines: Vec<Line> = humans
        .iter()
        .map(|h| {
            Line::from(vec![
                Span::raw(h.name.clone()),
                Span::styled(format!("  {}", h.email), dim),
            ])
        })
        .collect();

    render_info(lines)
}

async fn add(
    pool: &SqlitePool,
    name: &str,
    email: Option<&str>,
    org: Option<&str>,
    title: Option<&str>,
) -> CliResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    hypr_db_app::insert_human(
        pool,
        &id,
        name,
        email.unwrap_or(""),
        org.unwrap_or(""),
        title.unwrap_or(""),
    )
    .await
    .map_err(|e| CliError::operation_failed("insert human", e.to_string()))?;
    println!("{id}");
    Ok(())
}

async fn edit(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    email: Option<&str>,
    org_id: Option<&str>,
    job_title: Option<&str>,
    memo: Option<&str>,
) -> CliResult<()> {
    hypr_db_app::update_human(pool, id, name, email, org_id, job_title, memo)
        .await
        .map_err(|e| CliError::operation_failed("update human", e.to_string()))?;
    eprintln!("updated {id}");
    Ok(())
}

async fn show(pool: &SqlitePool, id: &str) -> CliResult<()> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);

    match hypr_db_app::get_human(pool, id).await {
        Ok(Some(h)) => {
            let mut lines = vec![
                Line::from(Span::styled(h.name, bold)),
                Line::from(vec![Span::styled("email      ", dim), Span::raw(h.email)]),
                Line::from(vec![Span::styled("org        ", dim), Span::raw(h.org_id)]),
                Line::from(vec![
                    Span::styled("title      ", dim),
                    Span::raw(h.job_title),
                ]),
                Line::from(vec![
                    Span::styled("created    ", dim),
                    Span::raw(h.created_at),
                ]),
            ];

            if let Ok(events) = hypr_db_app::list_events_by_human(pool, id).await {
                if !events.is_empty() {
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled("Recent events", bold)));
                    for event in events.into_iter().take(10) {
                        let date = if event.started_at.len() >= 16 {
                            event.started_at[..16].replace('T', " ")
                        } else {
                            event.started_at.replace('T', " ")
                        };
                        lines.push(Line::from(vec![
                            Span::styled(format!("{date}  "), dim),
                            Span::raw(event.title),
                        ]));
                    }
                }
            }

            if let Ok(participants) = hypr_db_app::list_meetings_by_human(pool, id).await {
                if !participants.is_empty() {
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled("Meetings", bold)));
                    for p in participants {
                        let title = hypr_db_app::get_meeting(pool, &p.meeting_id)
                            .await
                            .ok()
                            .flatten()
                            .and_then(|m| m.title)
                            .unwrap_or_default();
                        lines.push(Line::from(vec![
                            Span::styled(format!("{}  ", p.meeting_id), dim),
                            Span::raw(title),
                        ]));
                    }
                }
            }

            render_info(lines)
        }
        Ok(None) => Err(CliError::msg(format!("human '{id}' not found"))),
        Err(e) => Err(CliError::operation_failed("query", e.to_string())),
    }
}

async fn rm(pool: &SqlitePool, id: &str) -> CliResult<()> {
    hypr_db_app::delete_human(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("delete human", e.to_string()))?;
    eprintln!("deleted {id}");
    Ok(())
}

fn render_info(lines: Vec<Line<'static>>) -> CliResult<()> {
    let height = InlineBox::viewport_height(lines.len() as u16);
    hypr_cli_tui::render_inline(height, |frame| {
        let inner = InlineBox::render(frame);
        frame.render_widget(ratatui::widgets::Paragraph::new(lines), inner);
    })
    .map_err(|e| CliError::operation_failed("render", e.to_string()))
}
