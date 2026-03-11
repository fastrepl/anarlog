use std::path::Path;

use crate::{Error, ExportInput};

pub fn export_pdf(path: impl AsRef<Path>, input: impl Into<ExportInput>) -> Result<(), Error> {
    let input = input.into();
    let typst_content = crate::typst::build_typst_content(&input);
    let pdf_bytes = crate::typst::compile_to_pdf(&typst_content)?;
    std::fs::write(path.as_ref(), pdf_bytes)?;
    Ok(())
}
