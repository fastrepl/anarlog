use std::fs;
use std::fs::File;
use std::io;
use std::path::Path;

use crate::Error;

pub fn extract_zip(zip_path: impl AsRef<Path>, output_dir: impl AsRef<Path>) -> Result<(), Error> {
    let file = File::open(zip_path.as_ref())?;
    let mut archive = zip::ZipArchive::new(file)?;
    let output_dir = output_dir.as_ref();

    fs::create_dir_all(output_dir)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_name = entry.name().to_string();
        let Some(enclosed_path) = entry.enclosed_name() else {
            return Err(Error::OperationFailed(format!(
                "zip entry has invalid path: {entry_name}"
            )));
        };
        let destination = output_dir.join(enclosed_path);

        if entry.is_dir() {
            fs::create_dir_all(&destination)?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut out_file = File::create(&destination)?;
        io::copy(&mut entry, &mut out_file)?;
    }

    Ok(())
}
