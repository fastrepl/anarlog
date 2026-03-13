const COMMANDS: &[&str] = &[
    "event",
    "set_properties",
    "set_disabled",
    "is_disabled",
    "identify",
];

fn main() {
    println!("cargo:rerun-if-env-changed=POSTHOG_API_KEY");
    println!("cargo:rerun-if-env-changed=OUTLIT_PUBLIC_KEY");
    println!("cargo:rerun-if-env-changed=APP_VERSION");

    let app_version = match std::env::var("APP_VERSION") {
        Ok(v) if !v.is_empty() => v,
        _ => "dev".to_string(),
    };

    println!("cargo:rustc-env=APP_VERSION={}", app_version);

    tauri_plugin::Builder::new(COMMANDS).build();
}
