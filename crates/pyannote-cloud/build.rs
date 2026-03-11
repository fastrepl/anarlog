use progenitor_utils::OpenApiSpec;

const ALLOWED_PATH_PREFIXES: &[&str] = &["/v1/"];

fn main() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.gen.json");
    println!("cargo:rerun-if-changed={src}");

    OpenApiSpec::from_path(src)
        .retain_paths(ALLOWED_PATH_PREFIXES)
        .normalize_responses()
        .flatten_all_of()
        .remove_unreferenced_schemas()
        .write_filtered(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi-filtered.gen.json"),
        )
        .generate("codegen.rs");
}
