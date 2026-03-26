mod runner;
mod utils;

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use hypr_version::Version;

use crate::Result;
use crate::version::{DetectedVersion, version_from_name};

pub use runner::run;

pub trait Migration: Send + Sync {
    fn introduced_in(&self) -> &'static Version;

    fn applies_to(&self, _detected: &DetectedVersion) -> bool {
        true
    }

    fn run<'a>(&self, base_dir: &'a Path) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

macro_rules! migrations {
    ($($module:ident),* $(,)?) => {
        $(mod $module;)*

        fn all_migrations() -> Vec<&'static dyn Migration> {
            vec![$(&$module::Migrate),*]
        }

        pub fn latest_introduced_version() -> &'static Version {
            all_migrations().into_iter().map(|m| m.introduced_in()).max().expect("at least one migration must exist")
        }
    };
}

migrations! {
    v1_0_2_nightly_15_from_v0,
    v1_0_2_nightly_6_move_uuid_folders,
    v1_0_2_nightly_6_rename_transcript,
    v1_0_2_nightly_14_extract_from_sqlite,
    v1_0_4_nightly_2_repair_transcripts,
    v1_0_7_nightly_1_events_sync,
    v1_0_11_nightly_1_tracking_id_format,
}
