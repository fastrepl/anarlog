use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use hypr_version::Version;
use uuid::Uuid;

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

    let sessions_dir = base_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;

    let entries = std::fs::read_dir(base_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if Uuid::try_parse(name).is_err() {
            continue;
        }

        let target = sessions_dir.join(name);

        if target.exists() {
            continue;
        }

        std::fs::rename(&path, &target)?;
    }

    Ok(())
}
