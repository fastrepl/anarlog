use progenitor_utils::OpenApiSpec;

const ALLOWED_PATH_PREFIXES: &[&str] = &[
    "/calendar",
    "/feedback",
    "/nango",
    "/subscription",
    "/support",
];

const TYPE_REPLACEMENTS: &[(&str, &str)] = &[
    (
        "GoogleListCalendarsResponse",
        "hypr_google_calendar::ListCalendarsResponse",
    ),
    (
        "GoogleListEventsResponse",
        "hypr_google_calendar::ListEventsResponse",
    ),
    (
        "OutlookListCalendarsResponse",
        "hypr_outlook_calendar::ListCalendarsResponse",
    ),
    (
        "OutlookListEventsResponse",
        "hypr_outlook_calendar::ListEventsResponse",
    ),
];

fn main() {
    let src = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/api/openapi.gen.json"
    );
    println!("cargo:rerun-if-changed={src}");

    OpenApiSpec::from_path(src)
        .retain_paths(ALLOWED_PATH_PREFIXES)
        .normalize_responses()
        .flatten_all_of()
        .convert_31_to_30()
        .remove_unreferenced_schemas()
        .write_filtered(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.gen.json"))
        .generate_with_replacements("codegen.rs", TYPE_REPLACEMENTS);
}
