use crate::error::{CliError, CliResult};

const HELLO_URL: &str = "https://char.com";

pub fn run() -> CliResult<()> {
    if let Err(e) = open::that(HELLO_URL) {
        return Err(CliError::operation_failed(
            "open char.com",
            format!("{e}\nPlease visit: {HELLO_URL}"),
        ));
    }

    Ok(())
}
