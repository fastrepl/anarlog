use std::path::Path;

use crate::error::CliResult;

pub(super) fn paths(base: &Path, db_path: &Path, models_base: &Path) -> CliResult<()> {
    println!("base={}", base.display());
    println!("db_path={}", db_path.display());
    println!("models_base={}", models_base.display());
    Ok(())
}
