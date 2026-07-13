use std::path::Path;

use serde::Serialize;

use crate::{Error, Result};

pub fn json(value: &impl Serialize) -> Result<String> {
    serde_json::to_string_pretty(value)
        .map_err(|error| Error::operation("serialize output", error.to_string()))
}

pub fn emit(text: &str) {
    println!("{text}");
}

pub fn write_or_emit(text: &str, path: Option<&Path>) -> Result<()> {
    match path {
        Some(path) => std::fs::write(path, text)
            .map_err(|error| Error::operation("write export", error.to_string())),
        None => {
            emit(text);
            Ok(())
        }
    }
}
