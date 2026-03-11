pub mod auth;
pub mod batch;
pub mod cactus_server;
pub mod desktop;
pub mod entry;
pub mod listen;
pub mod model;

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Text,
    Json,
}
