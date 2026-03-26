use crate::error::{CliError, CliResult};

const BUG_URL: &str = "https://github.com/fastrepl/char/issues/new?labels=cli";

pub fn run() -> CliResult<()> {
    if let Err(e) = open::that(BUG_URL) {
        return Err(CliError::operation_failed(
            "open bug report page",
            format!("{e}\nPlease visit: {BUG_URL}"),
        ));
    }

    Ok(())
}
