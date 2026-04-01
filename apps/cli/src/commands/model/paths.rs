use std::path::Path;

use serde::Serialize;

use crate::cli::OutputFormat;
use crate::error::CliResult;

#[derive(Debug, PartialEq, Eq, Serialize)]
struct PathsOutput {
    base: String,
    db_path: String,
    models_base: String,
}

fn build_output(base: &Path, db_path: &Path, models_base: &Path) -> PathsOutput {
    PathsOutput {
        base: base.display().to_string(),
        db_path: db_path.display().to_string(),
        models_base: models_base.display().to_string(),
    }
}

fn format_pretty(output: &PathsOutput) -> String {
    format!(
        "base={}\ndb_path={}\nmodels_base={}",
        output.base, output.db_path, output.models_base
    )
}

pub(super) async fn paths(
    base: &Path,
    db_path: &Path,
    models_base: &Path,
    format: OutputFormat,
) -> CliResult<()> {
    let output = build_output(base, db_path, models_base);

    match format {
        OutputFormat::Pretty => println!("{}", format_pretty(&output)),
        OutputFormat::Json => crate::output::write_json(None, &output).await?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_pretty_matches_existing_text_shape() {
        let output = build_output(
            Path::new("/tmp/char"),
            Path::new("/tmp/char/app.db"),
            Path::new("/tmp/char/models"),
        );

        assert_eq!(
            format_pretty(&output),
            "base=/tmp/char\ndb_path=/tmp/char/app.db\nmodels_base=/tmp/char/models"
        );
    }

    #[test]
    fn json_output_uses_three_stable_keys() {
        let output = build_output(
            Path::new("/tmp/char"),
            Path::new("/tmp/char/app.db"),
            Path::new("/tmp/char/models"),
        );

        let json = serde_json::to_value(output).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "base": "/tmp/char",
                "db_path": "/tmp/char/app.db",
                "models_base": "/tmp/char/models",
            })
        );
    }
}
