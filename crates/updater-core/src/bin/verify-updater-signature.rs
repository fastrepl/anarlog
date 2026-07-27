use std::{env, fs};

use updater_core::verify_signature;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let artifact_path = args
        .next()
        .ok_or("usage: verify-updater-signature <artifact> <signature> <tauri-config>")?;
    let signature_path = args
        .next()
        .ok_or("usage: verify-updater-signature <artifact> <signature> <tauri-config>")?;
    let config_path = args
        .next()
        .ok_or("usage: verify-updater-signature <artifact> <signature> <tauri-config>")?;
    if args.next().is_some() {
        return Err("usage: verify-updater-signature <artifact> <signature> <tauri-config>".into());
    }

    let artifact = fs::read(artifact_path)?;
    let signature = fs::read_to_string(signature_path)?;
    let config: serde_json::Value = serde_json::from_slice(&fs::read(config_path)?)?;
    let pubkey = config
        .pointer("/plugins/updater/pubkey")
        .and_then(serde_json::Value::as_str)
        .ok_or("Tauri config is missing plugins.updater.pubkey")?;

    verify_signature(&artifact, signature.trim(), pubkey)?;
    println!("Updater signature verified");
    Ok(())
}
