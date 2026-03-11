use std::fs;
use std::path::Path;

use crate::eval::types::RunArtifact;
use crate::eval::util::sanitize_case_name;

pub(crate) fn write_artifact(dir: &Path, artifact: &RunArtifact) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|err| format!("failed to create artifact dir {}: {err}", dir.display()))?;
    let file_name = format!(
        "{}-{}-{}.json",
        sanitize_case_name(&artifact.case_name),
        sanitize_case_name(&artifact.model),
        artifact.created_at_ms
    );
    let path = dir.join(file_name);
    let bytes = serde_json::to_vec_pretty(artifact).map_err(|err| {
        format!(
            "failed to serialize artifact for {}: {err}",
            artifact.case_name
        )
    })?;
    fs::write(&path, bytes)
        .map_err(|err| format!("failed to write artifact {}: {err}", path.display()))?;
    Ok(())
}
