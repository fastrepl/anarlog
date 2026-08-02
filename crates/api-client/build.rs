use progenitor_utils::OpenApiSpec;
use std::path::PathBuf;

const ALLOWED_PATH_PREFIXES: &[&str] = &[
    "/calendar",
    "/nango",
    "/subscription",
    "/ticket",
    "/v1/cloud-api",
    "/v1/meetings",
    "/v1/sync-snapshots",
];

const TYPE_REPLACEMENTS: &[(&str, &str)] = &[
    (
        "GoogleListCalendarsResponse",
        "anlg_google_calendar::ListCalendarsResponse",
    ),
    (
        "GoogleListEventsResponse",
        "anlg_google_calendar::ListEventsResponse",
    ),
    (
        "OutlookListCalendarsResponse",
        "anlg_outlook_calendar::ListCalendarsResponse",
    ),
    (
        "OutlookListEventsResponse",
        "anlg_outlook_calendar::ListEventsResponse",
    ),
    ("CollectionPage", "anlg_ticket_interface::CollectionPage"),
    ("TicketPage", "anlg_ticket_interface::TicketPage"),
];

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"),
    );
    let src = manifest_dir.join("../../apps/api/openapi.gen.json");
    println!("cargo:rerun-if-changed={}", src.display());

    OpenApiSpec::from_path(&src)
        .retain_paths(ALLOWED_PATH_PREFIXES)
        .normalize_responses()
        .flatten_all_of()
        .convert_31_to_30()
        .remove_unreferenced_schemas()
        .write_filtered(manifest_dir.join("openapi.gen.json"))
        .generate_with_replacements("codegen.rs", TYPE_REPLACEMENTS);
}
