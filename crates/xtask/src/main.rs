mod mobile_bridge;
mod prepare_binaries;
mod toml_util;

use anyhow::{Result, bail};
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    match args.first().map(String::as_str) {
        Some("prepare-binaries") => prepare_binaries::prepare_binaries(),
        Some("mobile-bridge") => match args.get(1).map(String::as_str) {
            None | Some("ios") => mobile_bridge::mobile_bridge_ios(),
            Some(arg) => bail!("unknown mobile-bridge target: {arg}"),
        },
        Some("supabase-patch") => toml_util::supabase_patch(),
        Some("toml-set") => toml_util::toml_set(&args[1..]),
        None => {
            print_help();
            Ok(())
        }
        Some(cmd) => bail!("unknown xtask command: {cmd}"),
    }
}

fn print_help() {
    println!(
        "xtask\n\nUSAGE:\n    cargo xtask prepare-binaries\n    cargo xtask mobile-bridge [ios]\n    cargo xtask supabase-patch\n    cargo xtask toml-set <file> <key> <toml-value> [...]\n",
    );
}

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .expect("xtask crate lives under crates/")
}
