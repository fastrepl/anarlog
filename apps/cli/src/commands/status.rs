use std::io::IsTerminal;

use comfy_table::{Cell, Color, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};

use crate::config::desktop;
use crate::error::CliResult;

pub fn run() -> CliResult<()> {
    let paths = desktop::resolve_paths();
    let settings = desktop::load_settings(&paths.settings_path);

    eprintln!("settings: {}", paths.settings_path.display());
    eprintln!();

    let Some(settings) = settings else {
        eprintln!("No settings found. Run `char connect` to configure a provider.");
        return Ok(());
    };

    let is_tty = std::io::stdout().is_terminal();

    print_section(
        "STT",
        &settings.current_stt_provider,
        &settings.stt_providers,
        is_tty,
    );
    println!();
    print_section(
        "LLM",
        &settings.current_llm_provider,
        &settings.llm_providers,
        is_tty,
    );

    Ok(())
}

fn print_section(
    label: &str,
    current: &Option<String>,
    providers: &std::collections::HashMap<String, desktop::ProviderConfig>,
    is_tty: bool,
) {
    let current_str = current.as_deref().unwrap_or("(none)");
    eprintln!("{label} provider: {current_str}");

    if providers.is_empty() {
        eprintln!("  No {label} providers configured.");
        return;
    }

    if !is_tty {
        for (name, config) in providers {
            let active = if current.as_deref() == Some(name) {
                "*"
            } else {
                ""
            };
            let url = config.base_url.as_deref().unwrap_or("-");
            let key = if config.has_api_key { "yes" } else { "no" };
            println!("{}\t{}\t{}\t{}", active, name, url, key);
        }
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(["", "Provider", "Base URL", "API Key"]);

    let mut names: Vec<&String> = providers.keys().collect();
    names.sort();

    for name in names {
        let config = &providers[name];
        let active = if current.as_deref() == Some(name.as_str()) {
            Cell::new("*").fg(Color::Green)
        } else {
            Cell::new("")
        };
        let url = Cell::new(config.base_url.as_deref().unwrap_or("-"));
        let key = if config.has_api_key {
            Cell::new("yes").fg(Color::Green)
        } else {
            Cell::new("no").fg(Color::DarkGrey)
        };
        table.add_row([active, Cell::new(name), url, key]);
    }

    println!("{table}");
}
