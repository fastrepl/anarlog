use progenitor_utils::OpenApiSpec;

const ALLOWED_PATH_PREFIXES: &[&str] = &["/v1/"];

fn fix_pyannote_schema(spec: &mut serde_json::Value) {
    let pattern_path = "/components/schemas/GetMediaUploadUrl/properties/url/pattern";

    if let Some(pattern) = spec
        .pointer(pattern_path)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
    {
        if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 2 {
            *spec.pointer_mut(pattern_path).unwrap() =
                serde_json::Value::String(pattern[1..pattern.len() - 1].to_string());
        }
    }
}

fn main() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.gen.json");
    println!("cargo:rerun-if-changed={src}");

    let mut spec = OpenApiSpec::from_path(src);
    spec.retain_paths(ALLOWED_PATH_PREFIXES)
        .normalize_responses()
        .flatten_all_of()
        .remove_unreferenced_schemas();
    fix_pyannote_schema(spec.inner_mut());
    spec.write_filtered(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi-filtered.gen.json"),
    )
    .generate("codegen.rs");
}
