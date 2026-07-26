fn main() {
    let is_release = std::env::var("PROFILE").as_deref() == Ok("release");
    // CARGO_PRIMARY_PACKAGE is only set for the crate being directly compiled,
    // not for its (dev-)dependencies. A non-primary crate compiled in release
    // mode is a dev-dependency of `cargo test --release`, which is fine.
    let is_primary = std::env::var("CARGO_PRIMARY_PACKAGE").is_ok();

    if is_release && is_primary {
        panic!(
            "\n\nhypr-data is a test-only crate.\nDo not add it to [dependencies]; use [dev-dependencies] instead.\n"
        );
    }
}
