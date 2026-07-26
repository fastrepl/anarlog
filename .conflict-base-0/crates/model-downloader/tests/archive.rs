use std::fs;
use std::io::Write;

use model_downloader::Error;
use model_downloader::extract_zip;

fn write_zip(entries: &[(&str, &[u8])], zip_path: &std::path::Path) {
    let file = std::fs::File::create(zip_path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    for (name, content) in entries {
        writer.start_file(name, options).unwrap();
        writer.write_all(content).unwrap();
    }

    writer.finish().unwrap();
}

#[test]
fn extract_zip_extracts_nested_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let zip_path = temp_dir.path().join("model.zip");
    let output_dir = temp_dir.path().join("out");

    write_zip(
        &[
            ("model/config.json", br#"{"name":"demo"}"#),
            ("model/weights.bin", b"abc"),
        ],
        &zip_path,
    );

    extract_zip(&zip_path, &output_dir).unwrap();

    assert_eq!(
        fs::read_to_string(output_dir.join("model/config.json")).unwrap(),
        r#"{"name":"demo"}"#
    );
    assert_eq!(
        fs::read(output_dir.join("model/weights.bin")).unwrap(),
        b"abc"
    );
}

#[test]
fn extract_zip_rejects_invalid_paths() {
    let temp_dir = tempfile::tempdir().unwrap();
    let zip_path = temp_dir.path().join("model.zip");
    let output_dir = temp_dir.path().join("out");

    write_zip(&[("../evil.txt", b"bad")], &zip_path);

    let result = extract_zip(&zip_path, &output_dir);

    assert!(matches!(result, Err(Error::OperationFailed(_))));
    assert!(!temp_dir.path().join("evil.txt").exists());
}
