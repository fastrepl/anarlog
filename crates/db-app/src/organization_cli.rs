use sqlx::SqlitePool;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Text,
    Json,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// List all organizations
    List {
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,
    },
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
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,
    },
    /// Remove an organization
    Rm {
        #[arg(long)]
        id: String,
    },
}

pub async fn list(pool: &SqlitePool, format: OutputFormat) -> Result<(), crate::CrudCliError> {
    let rows = crate::list_organizations(pool)
        .await
        .map_err(|e| crate::CrudCliError::db("list organizations", e))?;
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rows).unwrap();
            println!("{json}");
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            if rows.is_empty() {
                eprintln!("No organizations found.");
            } else {
                for row in &rows {
                    println!("{}", row.name);
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
    let row = crate::get_organization(pool, id)
        .await
        .map_err(|e| crate::CrudCliError::db("query", e))?
        .ok_or_else(|| crate::CrudCliError::not_found("organization", id))?;
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&row).unwrap();
            println!("{json}");
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            println!("{}{}", format_args!("{:<11}", "name"), row.name);
            println!("{}{}", format_args!("{:<11}", "created"), row.created_at);
        }
    }
    Ok(())
}

pub async fn add(pool: &SqlitePool, name: &str) -> Result<String, crate::CrudCliError> {
    let id = uuid::Uuid::new_v4().to_string();
    crate::insert_organization(pool, &id, name)
        .await
        .map_err(|e| crate::CrudCliError::db("insert organization", e))?;
    Ok(id)
}

pub async fn edit(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
) -> Result<(), crate::CrudCliError> {
    crate::update_organization(pool, id, name)
        .await
        .map_err(|e| crate::CrudCliError::db("update organization", e))
}

pub async fn rm(pool: &SqlitePool, id: &str) -> Result<(), crate::CrudCliError> {
    crate::delete_organization(pool, id)
        .await
        .map_err(|e| crate::CrudCliError::db("delete organization", e))
}

pub async fn run(pool: &SqlitePool, command: Option<Commands>) -> Result<(), crate::CrudCliError> {
    match command {
        Some(Commands::List { format }) => list(pool, format).await,
        Some(Commands::Add { name }) => {
            let id = add(pool, &name).await?;
            println!("{id}");
            Ok(())
        }
        Some(Commands::Edit { id, name }) => {
            edit(pool, &id, name.as_deref()).await?;
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
