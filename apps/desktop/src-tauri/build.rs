fn main() {
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-fapple-link-rtlib");

    tauri_build::build()
}
