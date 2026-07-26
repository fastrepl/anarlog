fn main() {
    let is_release = std::env::var("PROFILE").as_deref() == Ok("release");
    let is_primary = std::env::var("CARGO_PRIMARY_PACKAGE").is_ok();

    if is_release && is_primary {
        panic!(
            "\n\naudio-snapshot is a test-only crate.\nDo not add it to [dependencies]; use [dev-dependencies] instead.\n"
        );
    }
}
