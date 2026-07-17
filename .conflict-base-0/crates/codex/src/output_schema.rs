use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::error::Error;

#[derive(Debug)]
pub(crate) struct OutputSchemaFile {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl OutputSchemaFile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn create_output_schema_file(
    schema: Option<&serde_json::Value>,
) -> Result<Option<OutputSchemaFile>, Error> {
    let Some(schema) = schema else {
        return Ok(None);
    };

    if !matches!(schema, serde_json::Value::Object(_)) {
        return Err(Error::InvalidOutputSchema);
    }

    let temp_dir = tempfile::tempdir().map_err(Error::OutputSchemaIo)?;
    let path = temp_dir.path().join("schema.json");
    let contents = serde_json::to_vec(schema)?;
    std::fs::write(&path, contents).map_err(Error::OutputSchemaIo)?;

    Ok(Some(OutputSchemaFile {
        _temp_dir: temp_dir,
        path,
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::create_output_schema_file;
    use crate::error::Error;

    #[test]
    fn accepts_object_schema_and_removes_tempdir_on_drop() {
        let schema_file = create_output_schema_file(Some(&json!({
            "type": "object",
            "properties": { "answer": { "type": "string" } }
        })))
        .expect("schema file should be created")
        .expect("schema file should exist");

        let schema_path = schema_file.path().to_path_buf();
        let schema_dir = schema_path
            .parent()
            .expect("schema file should have a parent directory")
            .to_path_buf();

        assert!(schema_path.is_file());
        drop(schema_file);
        assert!(!schema_dir.exists());
    }

    #[test]
    fn rejects_non_object_schema() {
        let error =
            create_output_schema_file(Some(&json!(["not", "an", "object"]))).expect_err("schema");
        assert!(matches!(error, Error::InvalidOutputSchema));
    }
}
