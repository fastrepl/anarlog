use sqlx::SqlitePool;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Text,
    Json,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// List all humans
    List {
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,
    },
    /// Add a new human
    Add {
        name: String,
        #[arg(long = "email")]
        email: Option<String>,
        #[arg(long = "org")]
        org_id: Option<String>,
        #[arg(long = "title")]
        job_title: Option<String>,
    },
    /// Edit a human
    Edit {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        email: Option<String>,
        #[arg(long = "org")]
        org_id: Option<String>,
        #[arg(long = "title")]
        job_title: Option<String>,
        #[arg(long)]
        memo: Option<String>,
    },
    /// Show details for a human
    Show {
        #[arg(long)]
        id: String,
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,
    },
    /// Remove a human
    Rm {
        #[arg(long)]
        id: String,
    },
}

pub async fn list(pool: &SqlitePool, format: OutputFormat) -> Result<(), crate::CrudCliError> {
    let rows = crate::list_humans(pool)
        .await
        .map_err(|e| crate::CrudCliError::db("list humans", e))?;
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rows).unwrap();
            println!("{json}");
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            if rows.is_empty() {
                eprintln!("No humans found.");
            } else {
                for row in &rows {
                    println!("{}\t{}", row.name, row.email);
                }
            }
        }
    }
    Ok(())
}

pub async fn show(
    pool: &SqlitePool,
    id: &str,
    format: OutputFormat,
) -> Result<(), crate::CrudCliError> {
    let row = crate::get_human(pool, id)
        .await
        .map_err(|e| crate::CrudCliError::db("query", e))?
        .ok_or_else(|| crate::CrudCliError::not_found("human", id))?;
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&row).unwrap();
            println!("{json}");
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            println!("{}{}", format_args!("{:<11}", "name"), row.name);
            println!("{}{}", format_args!("{:<11}", "email"), row.email);
            println!("{}{}", format_args!("{:<11}", "org"), row.org_id);
            println!("{}{}", format_args!("{:<11}", "title"), row.job_title);
            println!("{}{}", format_args!("{:<11}", "created"), row.created_at);
        }
    }
    Ok(())
}

pub async fn add(
    pool: &SqlitePool,
    name: &str,
    email: Option<&str>,
    org_id: Option<&str>,
    job_title: Option<&str>,
) -> Result<String, crate::CrudCliError> {
    let id = uuid::Uuid::new_v4().to_string();
    crate::insert_human(
        pool,
        &id,
        name,
        email.unwrap_or(""),
        org_id.unwrap_or(""),
        job_title.unwrap_or(""),
    )
    .await
    .map_err(|e| crate::CrudCliError::db("insert human", e))?;
    Ok(id)
}

pub async fn edit(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    email: Option<&str>,
    org_id: Option<&str>,
    job_title: Option<&str>,
    memo: Option<&str>,
) -> Result<(), crate::CrudCliError> {
    crate::update_human(pool, id, name, email, org_id, job_title, memo)
        .await
        .map_err(|e| crate::CrudCliError::db("update human", e))
}

pub async fn rm(pool: &SqlitePool, id: &str) -> Result<(), crate::CrudCliError> {
    crate::delete_human(pool, id)
        .await
        .map_err(|e| crate::CrudCliError::db("delete human", e))
}

pub async fn run(pool: &SqlitePool, command: Option<Commands>) -> Result<(), crate::CrudCliError> {
    match command {
        Some(Commands::List { format }) => list(pool, format).await,
        Some(Commands::Add {
            name,
            email,
            org_id,
            job_title,
        }) => {
            let id = add(
                pool,
                &name,
                email.as_deref(),
                org_id.as_deref(),
                job_title.as_deref(),
            )
            .await?;
            println!("{id}");
            Ok(())
        }
        Some(Commands::Edit {
            id,
            name,
            email,
            org_id,
            job_title,
            memo,
        }) => {
            edit(
                pool,
                &id,
                name.as_deref(),
                email.as_deref(),
                org_id.as_deref(),
                job_title.as_deref(),
                memo.as_deref(),
            )
            .await?;
            eprintln!("updated {id}");
            Ok(())
        }
        Some(Commands::Show { id, format }) => show(pool, &id, format).await,
        Some(Commands::Rm { id }) => {
            rm(pool, &id).await?;
            eprintln!("deleted {id}");
            Ok(())
        }
        None => list(pool, OutputFormat::Pretty).await,
    }
}
