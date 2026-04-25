use progenitor_utils::OpenApiSpec;

const ALLOWED_PATH_PREFIXES: &[&str] = &[
    "/calendar",
    "/feedback",
    "/nango",
    "/subscription",
    "/support",
    "/ticket",
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
    ("CollectionPage", "hypr_ticket_interface::CollectionPage"),
    ("TicketPage", "hypr_ticket_interface::TicketPage"),
];

fn main() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.gen.json");
    println!("cargo:rerun-if-changed={src}");

    OpenApiSpec::from_path(src)
        .retain_paths(ALLOWED_PATH_PREFIXES)
        .normalize_responses()
        .flatten_all_of()
        .remove_unreferenced_schemas()
        .generate_with_replacements("codegen.rs", TYPE_REPLACEMENTS);
}
