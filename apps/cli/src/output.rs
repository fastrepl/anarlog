use std::io::{IsTerminal, Write};
use std::path::Path;
use std::time::Duration;

use crate::error::{CliError, CliResult};

pub fn format_hhmmss(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

pub fn format_timestamp_ms(ms: i64) -> String {
    let total_secs = (ms / 1000).max(0);
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins >= 60 {
        let hours = mins / 60;
        let mins = mins % 60;
        format!("{hours:02}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    }
}

pub fn format_timestamp_secs(secs: f64) -> String {
    let total_tenths = (secs.max(0.0) * 10.0).round() as u64;
    let mins = total_tenths / 600;
    let s = (total_tenths % 600) / 10;
    let frac = total_tenths % 10;
    format!("{mins:02}:{s:02}.{frac}")
}

async fn ensure_parent_dirs(path: &Path) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CliError::operation_failed("create output directory", e.to_string()))?;
    }
    Ok(())
}

async fn write_bytes_to(output: Option<&Path>, bytes: Vec<u8>) -> CliResult<()> {
    if let Some(path) = output {
        ensure_parent_dirs(path).await?;
        tokio::fs::write(path, bytes)
            .await
            .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
        return Ok(());
    }

    std::io::stdout()
        .write_all(&bytes)
        .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
    std::io::stdout()
        .write_all(b"\n")
        .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
    Ok(())
}

pub async fn write_text(output: Option<&Path>, text: String) -> CliResult<()> {
    write_bytes_to(output, (text + "\n").into_bytes()).await
}

pub async fn write_json(output: Option<&Path>, value: &impl serde::Serialize) -> CliResult<()> {
    let bytes: Vec<u8> = if std::io::stdout().is_terminal() {
        serde_json::to_vec_pretty(value)
    } else {
        serde_json::to_vec(value)
    }
    .map_err(|e| CliError::operation_failed("serialize response", e.to_string()))?;

    write_bytes_to(output, bytes).await
}

#[cfg(test)]
mod tests {
    use super::format_timestamp_secs;

    #[test]
    fn format_timestamp_secs_handles_rounding_boundaries() {
        assert_eq!(format_timestamp_secs(5.94), "00:05.9");
        assert_eq!(format_timestamp_secs(5.95), "00:06.0");
        assert_eq!(format_timestamp_secs(59.95), "01:00.0");
        assert_eq!(format_timestamp_secs(0.0), "00:00.0");
    }
}
