use std::path::Path;

pub fn legacy_gguf_files(data_dir: &Path, models_dir: &Path) {
    let _ = std::fs::create_dir_all(models_dir);

    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("gguf") {
                if let Some(name) = path.file_name() {
                    let _ = std::fs::rename(&path, models_dir.join(name));
                }
            }
        }
    }
}
