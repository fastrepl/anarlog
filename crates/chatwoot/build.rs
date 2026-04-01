use progenitor_utils::OpenApiSpec;

const ALLOWED_PATH_PREFIXES: &[&str] = &[
    "/public/api/v1/inboxes/",
    "/api/v1/accounts/{account_id}/conversations",
    "/api/v1/accounts/{account_id}/contacts",
    "/api/v1/accounts/{account_id}/inboxes",
    "/api/v1/accounts/{account_id}/agent_bots",
    "/api/v1/accounts/{account_id}/webhooks",
];

const TYPE_REPLACEMENTS: &[(&str, &str)] = &[("ContactMeta", "crate::custom_types::ContactMeta")];

/// Chatwoot's OpenAPI spec has several type mismatches and mixed scalar unions
/// that don't survive codegen cleanly. Patch them here.
fn fix_chatwoot_schema_types(spec: &mut serde_json::Value) {
    let patches: &[(&str, &str, serde_json::Value)] = &[
        // Keep the schema parseable for codegen, then accept both strings and integers
        // in the handwritten replacement type at runtime.
        (
            "contact_meta",
            "current_page",
            serde_json::json!({"description": "Current page number", "type": "string"}),
        ),
        // message.updated_at is a unix timestamp like other message timestamps
        (
            "message",
            "updated_at",
            serde_json::json!({"description": "The time at which message was updated", "type": "integer"}),
        ),
        // Upstream docs and payloads do not agree on sender_type casing/variants.
        // Keep it as a plain string so runtime deserialization remains permissive.
        (
            "message",
            "sender_type",
            serde_json::json!({"description": "The type of the sender", "nullable": true, "type": "string"}),
        ),
    ];

    for (schema, field, value) in patches {
        if let Some(prop) =
            spec.pointer_mut(&format!("/components/schemas/{schema}/properties/{field}"))
        {
            *prop = value.clone();
        }
    }
}

fn main() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/swagger.gen.json");
    println!("cargo:rerun-if-changed={src}");

    let mut spec = OpenApiSpec::from_path(src);
    spec.retain_paths(ALLOWED_PATH_PREFIXES)
        .normalize_responses()
        .flatten_all_of()
        .convert_31_to_30()
        .remove_unreferenced_schemas();
    fix_chatwoot_schema_types(spec.inner_mut());
    spec.write_filtered(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.gen.json"))
        .generate_with_replacements("codegen.rs", TYPE_REPLACEMENTS);
}
