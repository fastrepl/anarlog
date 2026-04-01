use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use hypr_version::Version;

use super::version_from_name;
use crate::Result;
use crate::version::{DetectedVersion, InferredVersion};

pub struct Migrate;

impl super::Migration for Migrate {
    fn introduced_in(&self) -> &'static Version {
        version_from_name!()
    }

    fn applies_to(&self, detected: &DetectedVersion) -> bool {
        matches!(
            detected,
            DetectedVersion::Inferred(InferredVersion::V1_0_2NightlyEarly)
        )
    }

    fn run<'a>(&self, base_dir: &'a Path) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(run_inner(base_dir))
    }
}

async fn run_inner(base_dir: &Path) -> Result<()> {
    if !base_dir.exists() {
        return Ok(());
    }

    rename_recursively(base_dir)?;
    Ok(())
}

fn rename_recursively(dir: &Path) -> Result<()> {
    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            rename_recursively(&path)?;
        } else if path.file_name().and_then(|n| n.to_str()) == Some("_transcript.json") {
            let target = path.with_file_name("transcript.json");
            if target.exists() {
                std::fs::remove_file(&path)?;
            } else {
                std::fs::rename(&path, &target)?;
            }
        }
    }

    Ok(())
}
