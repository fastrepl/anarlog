use std::io::{IsTerminal, Write};
use std::path::Path;

use hypr_local_model::LocalModel;

use crate::error::{CliError, CliResult};

pub(super) async fn delete(model: LocalModel, models_base: &Path, force: bool) -> CliResult<()> {
    if !force && std::io::stderr().is_terminal() {
        eprint!("Delete {}? [y/N] ", model.display_name());
        std::io::stderr()
            .flush()
            .map_err(|e| CliError::operation_failed("prompt", e.to_string()))?;
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| CliError::operation_failed("read confirmation", e.to_string()))?;
        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Cancelled");
            return Ok(());
        }
    }

    let manager = super::make_manager(models_base, None);

    if let Err(e) = manager.delete(&model).await {
        return Err(CliError::operation_failed(
            "delete model",
            format!("{}: {e}", model.cli_name()),
        ));
    }

    eprintln!("Deleted {}", model.display_name());
    Ok(())
}
