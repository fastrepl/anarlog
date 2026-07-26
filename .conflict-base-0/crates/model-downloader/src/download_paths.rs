use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn generation_download_path(destination: &Path, generation: u64) -> PathBuf {
    let mut path = destination.to_path_buf();
    let suffix = format!(".part-{generation}");

    if let Some(file_name) = destination.file_name() {
        let mut generated_name = OsString::from(file_name);
        generated_name.push(suffix);
        path.set_file_name(generated_name);
    } else {
        path.push(format!("download{suffix}"));
    }

    path
}
