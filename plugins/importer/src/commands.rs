use crate::types::{
    ConnectedImportAuthorization, ConnectedImportCredentials, ConnectedImportSyncResult,
    ImportTextFile, MeetingPackageAudio,
};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use zip::{CompressionMethod, ZipArchive};

const MAX_IMPORT_FILE_COUNT: usize = 1_000;
const MAX_IMPORT_FILE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_TOTAL_IMPORT_BYTES: u64 = 100 * 1024 * 1024;
const MAX_MEETING_PACKAGE_BYTES: u64 = 545 * 1024 * 1024;
const MAX_MEETING_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_MEETING_AUDIO_BYTES: u64 = 512 * 1024 * 1024;
const MEETING_AUDIO_TEMP_PREFIX: &str = "anarlog-meeting-audio-";
const MEETING_AUDIO_EXTENSIONS: &[&str] =
    &["wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac"];
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "anlgmeeting",
    "csv",
    "json",
    "md",
    "markdown",
    "srt",
    "txt",
    "vtt",
];

#[tauri::command]
#[specta::specta]
pub async fn begin_connected_import(
    provider_id: String,
    mcp_state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
    cli_state: tauri::State<'_, crate::connected_cli::ConnectedImportCliState>,
) -> Result<ConnectedImportAuthorization, String> {
    if crate::connected_cli::is_cli_provider(&provider_id) {
        crate::connected_cli::begin_connection(&provider_id, &cli_state).await
    } else {
        crate::connected_mcp::begin_connection(&provider_id, &mcp_state).await
    }
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_connected_import(
    provider_id: String,
    mcp_state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
    cli_state: tauri::State<'_, crate::connected_cli::ConnectedImportCliState>,
) -> Result<bool, String> {
    if crate::connected_cli::is_cli_provider(&provider_id) {
        crate::connected_cli::cancel_connection(&provider_id, &cli_state).await
    } else {
        crate::connected_mcp::cancel_connection(&provider_id, &mcp_state).await
    }
}

#[tauri::command]
#[specta::specta]
pub async fn complete_connected_import(
    provider_id: String,
    mcp_state: tauri::State<'_, crate::connected_mcp::ConnectedImportOAuthState>,
    cli_state: tauri::State<'_, crate::connected_cli::ConnectedImportCliState>,
) -> Result<ConnectedImportCredentials, String> {
    if crate::connected_cli::is_cli_provider(&provider_id) {
        crate::connected_cli::complete_connection(&provider_id, &cli_state).await
    } else {
        crate::connected_mcp::complete_connection(&provider_id, &mcp_state).await
    }
}

#[tauri::command]
#[specta::specta]
pub async fn sync_connected_import(
    provider_id: String,
    credentials: ConnectedImportCredentials,
    known_meeting_ids: Vec<String>,
) -> Result<ConnectedImportSyncResult, String> {
    if crate::connected_cli::is_cli_provider(&provider_id) {
        crate::connected_cli::sync(&provider_id, credentials, known_meeting_ids).await
    } else {
        crate::connected_mcp::sync(&provider_id, credentials, known_meeting_ids).await
    }
}

#[tauri::command]
#[specta::specta]
pub async fn read_text_files(paths: Vec<String>) -> Result<Vec<ImportTextFile>, String> {
    if paths.len() > MAX_IMPORT_FILE_COUNT {
        return Err(format!(
            "select at most {MAX_IMPORT_FILE_COUNT} files per import"
        ));
    }

    let package_paths: Vec<_> = paths
        .iter()
        .filter(|path| {
            std::path::Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("anlgmeeting"))
        })
        .collect();
    if !package_paths.is_empty() {
        if paths.len() != 1 {
            return Err("import Anarlog meeting packages one at a time".to_string());
        }
        let path = package_paths[0];
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "invalid meeting package path".to_string())?
            .to_string();
        return read_meeting_package(path, name).map(|file| vec![file]);
    }

    let mut total_bytes = 0_u64;
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let path_buf = std::path::PathBuf::from(&path);
        let name = path_buf
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid import file path: {path}"))?
            .to_string();
        let extension = path_buf
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .ok_or_else(|| format!("{name} does not have a supported extension"))?;
        if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
            return Err(format!("{name} is not a supported meeting export"));
        }

        let metadata = std::fs::metadata(&path_buf)
            .map_err(|error| format!("could not inspect {name}: {error}"))?;
        if !metadata.is_file() {
            return Err(format!("{name} is not a file"));
        }
        if metadata.len() > MAX_IMPORT_FILE_BYTES {
            return Err(format!("{name} is larger than 20 MB"));
        }
        total_bytes = total_bytes.saturating_add(metadata.len());
        if total_bytes > MAX_TOTAL_IMPORT_BYTES {
            return Err("selected import files are larger than 100 MB total".to_string());
        }

        let content = std::fs::read_to_string(&path_buf)
            .map_err(|error| format!("could not read {name}: {error}"))?;
        files.push(ImportTextFile {
            path,
            name,
            content,
            meeting_audio: None,
        });
    }

    Ok(files)
}

#[tauri::command]
#[specta::specta]
pub fn cleanup_meeting_audio(path: String) -> Result<(), String> {
    let path = validate_meeting_audio_temp_path(&path)?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not remove temporary meeting audio: {error}")),
    }
}

fn read_meeting_package(path: &str, name: String) -> Result<ImportTextFile, String> {
    let path_buf = std::path::PathBuf::from(path);
    let metadata = std::fs::metadata(&path_buf)
        .map_err(|error| format!("could not inspect {name}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{name} is not a file"));
    }
    if metadata.len() > MAX_MEETING_PACKAGE_BYTES {
        return Err(format!("{name} is larger than the meeting package limit"));
    }

    let file = std::fs::File::open(&path_buf)
        .map_err(|error| format!("could not open {name}: {error}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|_| format!("{name} is not a valid Anarlog meeting package"))?;
    if !(1..=2).contains(&archive.len()) {
        return Err("the meeting package has an unsupported number of files".to_string());
    }
    let mut has_manifest = false;
    let mut has_audio = false;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| "the meeting package directory is corrupt".to_string())?;
        if entry.is_dir() || entry.is_symlink() || entry.compression() != CompressionMethod::Stored
        {
            return Err("the meeting package contains an unsupported file".to_string());
        }
        match entry.name() {
            "meeting.json" if !has_manifest => has_manifest = true,
            "audio/recording" if !has_audio => has_audio = true,
            _ => return Err("the meeting package contains an unexpected file".to_string()),
        }
    }
    if !has_manifest {
        return Err("the meeting package is missing meeting.json".to_string());
    }

    let manifest = {
        let mut entry = archive
            .by_name("meeting.json")
            .map_err(|_| "the meeting package is missing meeting.json".to_string())?;
        if entry.size() > MAX_MEETING_MANIFEST_BYTES {
            return Err("the meeting package manifest is too large".to_string());
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|_| "the meeting package manifest is corrupt".to_string())?;
        String::from_utf8(bytes)
            .map_err(|_| "the meeting package manifest is not UTF-8".to_string())?
    };
    let manifest_value: serde_json::Value = serde_json::from_str(&manifest)
        .map_err(|_| "the meeting package manifest is invalid JSON".to_string())?;
    if manifest_value
        .get("format")
        .and_then(|value| value.as_str())
        != Some("anarlog.meeting")
        || manifest_value
            .get("version")
            .and_then(|value| value.as_u64())
            != Some(1)
    {
        return Err("the meeting package format is not supported".to_string());
    }
    let audio_descriptor = manifest_value.get("audio");
    if has_audio != audio_descriptor.is_some_and(|value| !value.is_null()) {
        return Err("the meeting package audio does not match its manifest".to_string());
    }

    let meeting_audio = if has_audio {
        let descriptor = audio_descriptor
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| "the meeting package audio descriptor is invalid".to_string())?;
        let filename = descriptor
            .get("filename")
            .and_then(serde_json::Value::as_str)
            .filter(|value| safe_package_filename(value))
            .ok_or_else(|| "the meeting package audio filename is invalid".to_string())?
            .to_string();
        let content_type = descriptor
            .get("content_type")
            .and_then(serde_json::Value::as_str)
            .filter(|value| safe_package_content_type(value))
            .ok_or_else(|| "the meeting package audio type is invalid".to_string())?
            .to_string();
        let expected_size = descriptor
            .get("size_bytes")
            .and_then(serde_json::Value::as_u64)
            .filter(|size| *size > 0 && *size <= MAX_MEETING_AUDIO_BYTES)
            .ok_or_else(|| "the meeting package audio size is invalid".to_string())?;
        let expected_sha256 = descriptor
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| "the meeting package audio checksum is invalid".to_string())?;
        let extension = meeting_audio_extension(&filename, &content_type);
        let suffix = format!(".{extension}");
        let temp_file = tempfile::Builder::new()
            .prefix(MEETING_AUDIO_TEMP_PREFIX)
            .suffix(&suffix)
            .tempfile()
            .map_err(|error| format!("could not prepare meeting audio import: {error}"))?;
        let mut output = temp_file;
        let mut entry = archive
            .by_name("audio/recording")
            .map_err(|_| "the meeting package audio is missing".to_string())?;
        if entry.size() != expected_size || entry.size() > MAX_MEETING_AUDIO_BYTES {
            return Err("the meeting package audio size does not match its manifest".to_string());
        }
        let mut hasher = Sha256::new();
        let mut total = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let bytes_read = entry
                .read(&mut buffer)
                .map_err(|_| "the meeting package audio is corrupt".to_string())?;
            if bytes_read == 0 {
                break;
            }
            total += bytes_read as u64;
            if total > expected_size || total > MAX_MEETING_AUDIO_BYTES {
                return Err("the meeting package audio exceeds its declared size".to_string());
            }
            hasher.update(&buffer[..bytes_read]);
            output
                .write_all(&buffer[..bytes_read])
                .map_err(|error| format!("could not save meeting audio: {error}"))?;
        }
        if total != expected_size || sha256_hex(&hasher.finalize()) != expected_sha256 {
            return Err("the meeting package audio checksum does not match".to_string());
        }
        output
            .as_file()
            .sync_all()
            .map_err(|error| format!("could not finish meeting audio import: {error}"))?;
        let (file, audio_path) = output
            .keep()
            .map_err(|error| format!("could not keep meeting audio import: {}", error.error))?;
        drop(file);
        Some(MeetingPackageAudio {
            path: audio_path.to_string_lossy().into_owned(),
            filename,
            content_type,
            size_bytes: total,
            sha256: expected_sha256.to_string(),
        })
    } else {
        None
    };

    Ok(ImportTextFile {
        path: path.to_string(),
        name,
        content: manifest,
        meeting_audio,
    })
}

fn validate_meeting_audio_temp_path(path: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(path);
    let parent = std::env::temp_dir();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "invalid temporary meeting audio path".to_string())?;
    let Some((random, extension)) = file_name
        .strip_prefix(MEETING_AUDIO_TEMP_PREFIX)
        .and_then(|value| value.rsplit_once('.'))
    else {
        return Err("invalid temporary meeting audio path".to_string());
    };
    if random.is_empty()
        || !random
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
        || !MEETING_AUDIO_EXTENSIONS.contains(&extension)
        || path.parent() != Some(parent.as_path())
    {
        return Err("invalid temporary meeting audio path".to_string());
    }
    Ok(path)
}

fn meeting_audio_extension(filename: &str, content_type: &str) -> &'static str {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("wav") => return "wav",
        Some("mp3") => return "mp3",
        Some("ogg") => return "ogg",
        Some("mp4") => return "mp4",
        Some("m4a") => return "m4a",
        Some("flac") => return "flac",
        Some("webm") => return "webm",
        Some("aac") => return "aac",
        _ => {}
    }
    match content_type.split(';').next().unwrap_or_default().trim() {
        "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" => "ogg",
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" | "audio/quicktime" | "audio/x-quicktime"
        | "video/mp4" | "video/quicktime" => "m4a",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/webm" => "webm",
        "audio/aac" | "audio/x-aac" => "aac",
        _ => "mp3",
    }
}

fn safe_package_filename(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value != "."
        && value != ".."
        && !value
            .chars()
            .any(|character| character == '/' || character == '\\' || character.is_control())
}

fn safe_package_content_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod meeting_package_tests {
    use super::*;
    use zip::write::{SimpleFileOptions, ZipWriter};

    const AUDIO: &[u8] = b"meeting audio";

    fn archive(manifest: &str, audio: Option<&[u8]>) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut writer = ZipWriter::new(file.reopen().unwrap());
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        writer.start_file("meeting.json", options).unwrap();
        writer.write_all(manifest.as_bytes()).unwrap();
        if let Some(audio) = audio {
            writer.start_file("audio/recording", options).unwrap();
            writer.write_all(audio).unwrap();
        }
        writer.finish().unwrap();
        file
    }

    fn manifest(audio: Option<&[u8]>) -> String {
        let audio = audio.map_or_else(
            || "null".to_string(),
            |bytes| {
                format!(
                    "{{\"content_type\":\"audio/wav\",\"filename\":\"meeting.wav\",\"sha256\":\"{}\",\"size_bytes\":{}}}",
                    sha256_hex(&Sha256::digest(bytes)),
                    bytes.len()
                )
            },
        );
        format!("{{\"format\":\"anarlog.meeting\",\"version\":1,\"audio\":{audio}}}")
    }

    #[test]
    fn streams_and_verifies_meeting_audio_from_archive() {
        let package = archive(&manifest(Some(AUDIO)), Some(AUDIO));
        let path = package.path().to_string_lossy().into_owned();
        let imported = read_meeting_package(&path, "meeting.anlgmeeting".to_string()).unwrap();
        let audio = imported.meeting_audio.unwrap();
        assert_eq!(imported.content, manifest(Some(AUDIO)));
        assert_eq!(std::fs::read(&audio.path).unwrap(), AUDIO);
        assert_eq!(audio.filename, "meeting.wav");
        assert_eq!(audio.content_type, "audio/wav");
        assert_eq!(audio.size_bytes, AUDIO.len() as u64);
        cleanup_meeting_audio(audio.path).unwrap();
    }

    #[test]
    fn accepts_text_only_meeting_package() {
        let package = archive(&manifest(None), None);
        let imported = read_meeting_package(
            &package.path().to_string_lossy(),
            "meeting.anlgmeeting".to_string(),
        )
        .unwrap();
        assert!(imported.meeting_audio.is_none());
    }

    #[test]
    fn rejects_audio_that_does_not_match_manifest_checksum() {
        let package = archive(&manifest(Some(b"different audio")), Some(AUDIO));
        assert!(
            read_meeting_package(
                &package.path().to_string_lossy(),
                "meeting.anlgmeeting".to_string()
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_archive_paths_outside_the_meeting_format() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut writer = ZipWriter::new(file.reopen().unwrap());
        writer
            .start_file(
                "../outside",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(b"bad").unwrap();
        writer.finish().unwrap();
        assert!(
            read_meeting_package(
                &file.path().to_string_lossy(),
                "meeting.anlgmeeting".to_string()
            )
            .is_err()
        );
    }
}
