use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{Args, Commands, TemplateCommand, UpsertTemplateArgs};
use crate::output::print_json;
use crate::{Error, Result};

#[derive(Serialize)]
struct Status<'a> {
    status: &'a str,
    id: &'a str,
}

#[derive(Serialize)]
struct TemplateOutput {
    id: String,
    title: String,
    description: String,
    pinned: bool,
    pin_order: Option<i64>,
    category: Option<String>,
    targets_json: Option<String>,
    sections_json: String,
    created_at: String,
    updated_at: String,
}

pub async fn run(args: Args) -> Result<()> {
    let db_path = resolve_db_path(args.base.as_deref(), args.db_path.as_deref());

    let db = hypr_db_core::Db::connect_local_plain(&db_path)
        .await
        .map_err(|e| Error::operation_failed("open database", e.to_string()))?;

    hypr_db_migrate::migrate(&db, hypr_db_app::schema())
        .await
        .map_err(|e| Error::operation_failed("migrate database", e.to_string()))?;

    match args.command {
        Commands::Templates { command } => run_templates(db.pool(), command).await,
    }
}

fn default_base_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("char")
}

fn resolve_db_path(base: Option<&Path>, db_path: Option<&Path>) -> PathBuf {
    match db_path {
        Some(path) => path.to_path_buf(),
        None => base
            .map(Path::to_path_buf)
            .unwrap_or_else(default_base_dir)
            .join("app.db"),
    }
}

async fn run_templates(pool: &sqlx::SqlitePool, command: TemplateCommand) -> Result<()> {
    match command {
        TemplateCommand::List => {
            let rows = hypr_db_app::list_templates(pool)
                .await
                .map_err(|e| Error::operation_failed("list templates", e.to_string()))?;
            let rows: Vec<_> = rows.into_iter().map(TemplateOutput::from).collect();
            print_json(&rows)
        }
        TemplateCommand::Get { id } => {
            let row = hypr_db_app::get_template(pool, &id)
                .await
                .map_err(|e| Error::operation_failed("get template", e.to_string()))?
                .ok_or_else(|| Error::not_found(format!("template '{id}'")))?;
            print_json(&TemplateOutput::from(row))
        }
        TemplateCommand::Upsert(args) => upsert_template(pool, args).await,
        TemplateCommand::Delete { id } => {
            hypr_db_app::delete_template(pool, &id)
                .await
                .map_err(|e| Error::operation_failed("delete template", e.to_string()))?;
            print_json(&Status {
                status: "deleted",
                id: &id,
            })
        }
    }
}

impl From<hypr_db_app::TemplateRow> for TemplateOutput {
    fn from(value: hypr_db_app::TemplateRow) -> Self {
        Self {
            id: value.id,
            title: value.title,
            description: value.description,
            pinned: value.pinned,
            pin_order: value.pin_order,
            category: value.category,
            targets_json: value.targets_json,
            sections_json: value.sections_json,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

async fn upsert_template(pool: &sqlx::SqlitePool, args: UpsertTemplateArgs) -> Result<()> {
    let id = args.id;

    hypr_db_app::upsert_template(
        pool,
        hypr_db_app::UpsertTemplate {
            id: &id,
            title: &args.title,
            description: &args.description,
            pinned: args.pinned,
            pin_order: args.pin_order,
            category: args.category.as_deref(),
            targets_json: args.targets_json.as_deref(),
            sections_json: &args.sections_json,
        },
    )
    .await
    .map_err(|e| Error::operation_failed("upsert template", e.to_string()))?;

    print_json(&Status {
        status: "upserted",
        id: &id,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::cli::{Commands, TemplateCommand};

    #[test]
    fn resolve_db_path_prefers_explicit_db_path() {
        let resolved = resolve_db_path(
            Some(Path::new("/tmp/char")),
            Some(Path::new("/tmp/custom/app.db")),
        );
        assert_eq!(resolved, PathBuf::from("/tmp/custom/app.db"));
    }

    #[test]
    fn resolve_db_path_defaults_to_base_app_db() {
        let resolved = resolve_db_path(Some(Path::new("/tmp/char")), None);
        assert_eq!(resolved, PathBuf::from("/tmp/char/app.db"));
    }

    #[tokio::test]
    async fn run_migrates_and_roundtrips_templates() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("templates.db");

        run(Args {
            base: None,
            db_path: Some(db_path.clone()),
            command: Commands::Templates {
                command: TemplateCommand::Upsert(UpsertTemplateArgs {
                    id: "template-1".to_string(),
                    title: "Standup".to_string(),
                    description: "Daily sync".to_string(),
                    pinned: true,
                    pin_order: Some(1),
                    category: Some("meetings".to_string()),
                    targets_json: Some("[\"engineering\"]".to_string()),
                    sections_json: "[{\"title\":\"Notes\"}]".to_string(),
                }),
            },
        })
        .await
        .unwrap();

        let db = hypr_db_core::Db::connect_local_plain(&db_path)
            .await
            .unwrap();
        let row = hypr_db_app::get_template(db.pool(), "template-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(row.title, "Standup");
        assert!(row.pinned);
    }

    #[tokio::test]
    async fn run_returns_not_found_for_missing_template() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("templates.db");

        let error = run(Args {
            base: None,
            db_path: Some(db_path),
            command: Commands::Templates {
                command: TemplateCommand::Get {
                    id: "missing".to_string(),
                },
            },
        })
        .await
        .unwrap_err();

        assert_eq!(error.to_string(), "template 'missing' not found");
    }

    #[tokio::test]
    async fn run_deletes_templates() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("templates.db");
        let db = hypr_db_core::Db::connect_local_plain(&db_path)
            .await
            .unwrap();
        hypr_db_migrate::migrate(&db, hypr_db_app::schema())
            .await
            .unwrap();
        hypr_db_app::upsert_template(
            db.pool(),
            hypr_db_app::UpsertTemplate {
                id: "template-1",
                title: "Standup",
                description: "",
                pinned: false,
                pin_order: None,
                category: None,
                targets_json: None,
                sections_json: "[]",
            },
        )
        .await
        .unwrap();
        drop(db);

        run(Args {
            base: None,
            db_path: Some(db_path.clone()),
            command: Commands::Templates {
                command: TemplateCommand::Delete {
                    id: "template-1".to_string(),
                },
            },
        })
        .await
        .unwrap();

        let db = hypr_db_core::Db::connect_local_plain(&db_path)
            .await
            .unwrap();
        let row = hypr_db_app::get_template(db.pool(), "template-1")
            .await
            .unwrap();
        assert!(row.is_none());
    }
}
