use std::{fs, path::Path};

#[path = "src/types.rs"]
mod types;

#[allow(dead_code)]
#[derive(schemars::JsonSchema)]
struct ActivityCaptureInterfaceSchema {
    capture_policy: types::CapturePolicy,
    app_identity: types::AppIdentity,
    browser_context: types::BrowserContext,
    sanitized_browser_url: types::SanitizedBrowserUrl,
    capture_candidate: types::CaptureCandidate,
    capture_decision: types::CaptureDecision,
    snapshot_spec: types::SnapshotSpec,
    snapshot: types::Snapshot,
    event: types::Event,
    transition: types::Transition,
    capabilities: types::Capabilities,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/types.rs");

    let schema = schemars::schema_for!(ActivityCaptureInterfaceSchema);
    let schema_json = serde_json::to_string_pretty(&schema).expect("serialize schema");
    let output_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema.gen.json");

    if fs::read_to_string(&output_path).ok().as_deref() == Some(&schema_json) {
        return;
    }

    fs::write(output_path, schema_json).expect("write activity capture schema");
}
