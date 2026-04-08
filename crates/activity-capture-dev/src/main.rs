mod app;
mod event_row;
mod export;
mod formatting;
mod options;
mod theme;
mod ui;

use std::{io::IsTerminal, process};

use clap::Parser;

use crate::{app::run, options::Options};

fn main() {
    let options = Options::parse();
    let color_enabled = !options.no_color && std::io::stdout().is_terminal();

    if let Err(error) = run(options, color_enabled) {
        eprintln!("snapshot error: {error}");
        process::exit(1);
    }
}
