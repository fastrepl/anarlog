use crate::error::{CliError, CliResult};

const AUTH_URL: &str = "https://char.com/auth";

pub fn run() -> CliResult<()> {
    if let Err(e) = open::that(AUTH_URL) {
        return Err(CliError::operation_failed(
            "open auth page",
            format!("{e}\nPlease visit: {AUTH_URL}"),
        ));
    }

    Ok(())
}
