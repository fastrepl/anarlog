use std::path::PathBuf;

use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[arg(long, env = "CHAR_BASE", hide_env_values = true, value_name = "DIR")]
    pub base: Option<PathBuf>,

    #[arg(long, value_name = "FILE")]
    pub db_path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage templates stored in the desktop SQLite database
    Templates {
        #[command(subcommand)]
        command: TemplateCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum TemplateCommand {
    /// List templates
    List,
    /// Fetch a template by id
    Get { id: String },
    /// Insert or update a template
    Upsert(UpsertTemplateArgs),
    /// Delete a template by id
    Delete { id: String },
}

#[derive(clap::Args, Debug)]
pub struct UpsertTemplateArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub title: String,
    #[arg(long, default_value = "")]
    pub description: String,
    #[arg(long, default_value_t = false)]
    pub pinned: bool,
    #[arg(long)]
    pub pin_order: Option<i64>,
    #[arg(long)]
    pub category: Option<String>,
    #[arg(long)]
    pub targets_json: Option<String>,
    #[arg(long)]
    pub sections_json: String,
}

#[derive(Parser)]
struct TestCli {
    #[command(subcommand)]
    command: RootCommands,
}

#[derive(Subcommand)]
enum RootCommands {
    Db {
        #[command(flatten)]
        args: Args,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn args_help_mentions_templates() {
        let mut command = TestCli::command();
        let mut db = command.find_subcommand_mut("db").unwrap().clone();
        let mut bytes = Vec::new();
        db.write_long_help(&mut bytes).unwrap();
        let help = String::from_utf8(bytes).unwrap();

        assert!(help.contains("templates"));
        assert!(help.contains("--base <DIR>"));
        assert!(help.contains("--db-path <FILE>"));
    }

    #[test]
    fn clap_parses_template_subcommands() {
        let cli = TestCli::parse_from([
            "char",
            "db",
            "--base",
            "/tmp/char",
            "templates",
            "get",
            "template-1",
        ]);

        let RootCommands::Db { args } = cli.command;
        assert_eq!(
            args.base.as_deref(),
            Some(std::path::Path::new("/tmp/char"))
        );
        assert!(args.db_path.is_none());
        match args.command {
            Commands::Templates { command } => match command {
                TemplateCommand::Get { id } => assert_eq!(id, "template-1"),
                _ => panic!("expected get subcommand"),
            },
        }
    }
}
