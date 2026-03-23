use progenitor_utils::OpenApiSpec;

const ALLOWED_PATH_PREFIXES: &[&str] = &[
    "/public/api/v1/inboxes/",
    "/api/v1/accounts/{account_id}/conversations",
    "/api/v1/accounts/{account_id}/contacts",
    "/api/v1/accounts/{account_id}/inboxes",
    "/api/v1/accounts/{account_id}/agent_bots",
    "/api/v1/accounts/{account_id}/webhooks",
];

fn main() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/swagger.gen.json");
    println!("cargo:rerun-if-changed={src}");

    OpenApiSpec::from_path(src)
        .retain_paths(ALLOWED_PATH_PREFIXES)
        .normalize_responses()
        .flatten_all_of()
        .remove_unreferenced_schemas()
        .write_filtered(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.gen.json"))
        .generate("codegen.rs");
}
