use clap::Subcommand;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use sqlx::SqlitePool;

use crate::error::{CliError, CliResult};
use crate::widgets::InlineBox;

#[derive(Subcommand)]
pub enum Commands {
    /// List all organizations
    List,
    /// Add a new organization
    Add { name: String },
    /// Edit an organization
    Edit {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Show details for an organization
    Show {
        #[arg(long)]
        id: String,
    },
    /// Remove an organization
    Rm {
        #[arg(long)]
        id: String,
    },
}

pub async fn run(pool: &SqlitePool, command: Option<Commands>) -> CliResult<()> {
    match command {
        Some(Commands::Add { name }) => add(pool, &name).await,
        Some(Commands::Edit { id, name }) => edit(pool, &id, name.as_deref()).await,
        Some(Commands::Show { id }) => show(pool, &id).await,
        Some(Commands::Rm { id }) => rm(pool, &id).await,
        Some(Commands::List) | None => list(pool).await,
    }
}

async fn list(pool: &SqlitePool) -> CliResult<()> {
    let dim = Style::default().add_modifier(Modifier::DIM);

    let orgs = hypr_db_app::list_organizations(pool)
        .await
        .map_err(|e| CliError::operation_failed("list organizations", e.to_string()))?;

    if orgs.is_empty() {
        let lines = vec![Line::from(Span::styled("No organizations found.", dim))];
        return render_info(lines);
    }

    let lines: Vec<Line> = orgs
        .into_iter()
        .map(|org| Line::from(Span::raw(org.name)))
        .collect();

    render_info(lines)
}

async fn add(pool: &SqlitePool, name: &str) -> CliResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    hypr_db_app::insert_organization(pool, &id, name)
        .await
        .map_err(|e| CliError::operation_failed("insert organization", e.to_string()))?;
    println!("{id}");
    Ok(())
}

async fn edit(pool: &SqlitePool, id: &str, name: Option<&str>) -> CliResult<()> {
    hypr_db_app::update_organization(pool, id, name)
        .await
        .map_err(|e| CliError::operation_failed("update organization", e.to_string()))?;
    eprintln!("updated {id}");
    Ok(())
}

async fn show(pool: &SqlitePool, id: &str) -> CliResult<()> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);

    match hypr_db_app::get_organization(pool, id).await {
        Ok(Some(org)) => {
            let mut lines = vec![
                Line::from(Span::styled(org.name, bold)),
                Line::from(vec![
                    Span::styled("created    ", dim),
                    Span::raw(org.created_at),
                ]),
            ];

            if let Ok(events) = hypr_db_app::list_events_by_org(pool, id).await {
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

            if let Ok(humans) = hypr_db_app::list_humans_by_org(pool, id).await {
                if !humans.is_empty() {
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled("Members", bold)));
                    for h in humans {
                        lines.push(Line::from(vec![
                            Span::raw(h.name),
                            Span::styled(format!("  {}", h.email), dim),
                        ]));
                    }
                }
            }

            render_info(lines)
        }
        Ok(None) => Err(CliError::msg(format!("organization '{id}' not found"))),
        Err(e) => Err(CliError::operation_failed("query", e.to_string())),
    }
}

async fn rm(pool: &SqlitePool, id: &str) -> CliResult<()> {
    hypr_db_app::delete_organization(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("delete organization", e.to_string()))?;
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
