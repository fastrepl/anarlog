use std::{
    fs,
    io::Cursor,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Clone)]
pub(crate) struct StagedMacosUpdate {
    pub(crate) current_app_path: PathBuf,
    pub(crate) staged_app_path: PathBuf,
    pub(crate) target_app_path: PathBuf,
    pub(crate) current_backup_path: PathBuf,
    pub(crate) stage_dir: PathBuf,
}

#[derive(Debug)]
struct ExtractedBundle {
    name: String,
    path: PathBuf,
}

struct MacInstallPaths {
    current_app_path: PathBuf,
    staged_app_path: PathBuf,
    target_app_path: PathBuf,
    current_backup_path: PathBuf,
}

impl MacInstallPaths {
    fn new(
        current_app_path: PathBuf,
        backup_dir: &Path,
        extracted_bundle: ExtractedBundle,
    ) -> Result<Self, crate::Error> {
        let target_app_path = install_target_path(
            &current_app_path,
            target_bundle_name(&current_app_path, &extracted_bundle.name),
        )?;
        let current_backup_path = backup_dir.join(
            current_app_path
                .file_name()
                .ok_or(crate::Error::FailedToDetermineCurrentAppPath)?,
        );

        Ok(Self {
            staged_app_path: extracted_bundle.path,
            current_app_path,
            target_app_path,
            current_backup_path,
        })
    }

    fn staged_update(&self, stage_dir: PathBuf) -> StagedMacosUpdate {
        StagedMacosUpdate {
            current_app_path: self.current_app_path.clone(),
            staged_app_path: self.staged_app_path.clone(),
            target_app_path: self.target_app_path.clone(),
            current_backup_path: self.current_backup_path.clone(),
            stage_dir,
        }
    }
}

pub(crate) fn stage_macos_update(
    bytes: &[u8],
    stage_dir: &Path,
) -> Result<StagedMacosUpdate, crate::Error> {
    let current_app_path = current_app_bundle_path()?;
    stage_macos_update_for_current_app(bytes, current_app_path, stage_dir)
}

fn stage_macos_update_for_current_app(
    bytes: &[u8],
    current_app_path: PathBuf,
    stage_dir: &Path,
) -> Result<StagedMacosUpdate, crate::Error> {
    let backup_dir = stage_dir.join("backup");
    let extract_dir = stage_dir.join("staged");
    fs::create_dir_all(&backup_dir)?;
    fs::create_dir_all(&extract_dir)?;

    let extracted_bundle = extract_macos_bundle(bytes, &extract_dir)?;
    let paths = MacInstallPaths::new(current_app_path, &backup_dir, extracted_bundle)?;
    ensure_target_app_path_available(&paths)?;

    tracing::info!(
        current_app_path = %paths.current_app_path.display(),
        target_app_path = %paths.target_app_path.display(),
        staged_app_path = %paths.staged_app_path.display(),
        "staging macOS update"
    );

    Ok(paths.staged_update(stage_dir.to_path_buf()))
}

fn install_target_path(
    current_app_path: &Path,
    target_bundle_name: &str,
) -> Result<PathBuf, crate::Error> {
    let parent = current_app_path
        .parent()
        .ok_or(crate::Error::FailedToDetermineTargetAppPath)?;
    Ok(parent.join(target_bundle_name))
}

fn target_bundle_name<'a>(current_app_path: &'a Path, extracted_bundle_name: &'a str) -> &'a str {
    match current_app_path.file_name().and_then(|name| name.to_str()) {
        Some("Hyprnote.app") => "Char.app",
        Some("Hyprnote Nightly.app") => "Char Nightly.app",
        Some("Hyprnote Staging.app") => "Char Staging.app",
        _ => extracted_bundle_name,
    }
}

pub(crate) fn schedule_macos_update_after_exit(
    current_pid: u32,
    staged_update: StagedMacosUpdate,
) -> Result<(), crate::Error> {
    let mut command = build_macos_update_apply_command(current_pid, &staged_update);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());

    command.spawn().map_err(|err| {
        tracing::error!(
            current_pid,
            target_app_path = %staged_update.target_app_path.display(),
            error = %err,
            "failed to schedule staged macOS update apply"
        );
        crate::Error::FailedToScheduleInstalledAppLaunch {
            path: staged_update.target_app_path.display().to_string(),
            details: err.to_string(),
        }
    })?;

    Ok(())
}

fn build_macos_update_apply_command(
    current_pid: u32,
    staged_update: &StagedMacosUpdate,
) -> Command {
    let paths = MacInstallPaths {
        current_app_path: staged_update.current_app_path.clone(),
        staged_app_path: staged_update.staged_app_path.clone(),
        target_app_path: staged_update.target_app_path.clone(),
        current_backup_path: staged_update.current_backup_path.clone(),
    };
    let apply_script = format!(
        r#"while kill -0 "$1" 2>/dev/null; do sleep 0.1; done;
apply_update() {{
  if [ {current} != {target} ] && [ -e {target} ]; then
    echo 'target bundle already exists' >&2
    exit 1
  fi

  if ! mv -f {current} {backup}; then
    return 1
  fi

  if ! mv -f {staged} {target}; then
    if [ -e {backup} ]; then mv -f {backup} {current}; fi
    exit 1
  fi
}}

if ! apply_update; then
  osascript -e {authorization} || exit 1
fi

touch {target} >/dev/null 2>&1 || true
if open -n {target}; then
  rm -rf {stage_dir}
fi"#,
        current = shell_quote(&paths.current_app_path),
        staged = shell_quote(&paths.staged_app_path),
        target = shell_quote(&paths.target_app_path),
        backup = shell_quote(&paths.current_backup_path),
        stage_dir = shell_quote(&staged_update.stage_dir),
        authorization = do_shell_script_with_privileges(&authorization_script(&paths)),
    );
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(apply_script)
        .arg("sh")
        .arg(current_pid.to_string());
    command
}

fn ensure_target_app_path_available(paths: &MacInstallPaths) -> Result<(), crate::Error> {
    if paths.target_app_path != paths.current_app_path && paths.target_app_path.exists() {
        return Err(crate::Error::MacosTargetBundleConflict {
            path: paths.target_app_path.display().to_string(),
        });
    }

    Ok(())
}

fn authorization_script(paths: &MacInstallPaths) -> String {
    if paths.current_app_path == paths.target_app_path {
        format!(
            "set -e; \
             if ! mv -f {current} {current_backup}; then exit 1; fi; \
             if ! mv -f {staged} {target}; then \
               if [ -e {current_backup} ]; then mv -f {current_backup} {current}; fi; \
               exit 1; \
             fi",
            current = shell_quote(&paths.current_app_path),
            current_backup = shell_quote(&paths.current_backup_path),
            staged = shell_quote(&paths.staged_app_path),
            target = shell_quote(&paths.target_app_path),
        )
    } else {
        format!(
            "set -e; \
             if [ -e {target} ]; then echo 'target bundle already exists' >&2; exit 1; fi; \
             if ! mv -f {current} {current_backup}; then \
               exit 1; \
             fi; \
             if ! mv -f {staged} {target}; then \
               if [ -e {current_backup} ]; then mv -f {current_backup} {current}; fi; \
               exit 1; \
             fi",
            current = shell_quote(&paths.current_app_path),
            current_backup = shell_quote(&paths.current_backup_path),
            staged = shell_quote(&paths.staged_app_path),
            target = shell_quote(&paths.target_app_path),
        )
    }
}

fn current_app_bundle_path() -> Result<PathBuf, crate::Error> {
    let executable_path = tauri::utils::platform::current_exe()?;
    current_app_bundle_path_from_executable(&executable_path)
}

fn current_app_bundle_path_from_executable(
    executable_path: &Path,
) -> Result<PathBuf, crate::Error> {
    let app_path = executable_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or(crate::Error::FailedToDetermineCurrentAppPath)?;

    if app_path.extension().and_then(|ext| ext.to_str()) != Some("app") {
        return Err(crate::Error::FailedToDetermineCurrentAppPath);
    }

    Ok(app_path.to_path_buf())
}

fn extract_macos_bundle(bytes: &[u8], target_dir: &Path) -> Result<ExtractedBundle, crate::Error> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let mut root_name: Option<String> = None;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?;
        let entry_root = archive_entry_root(&entry_path)?;
        validate_bundle_root(&mut root_name, entry_root)?;

        if !entry.unpack_in(target_dir)? {
            return Err(crate::Error::InvalidUpdateArchive(
                "archive entry contains an invalid relative path".into(),
            ));
        }
    }

    let name =
        root_name.ok_or_else(|| crate::Error::InvalidUpdateArchive("archive is empty".into()))?;

    Ok(ExtractedBundle {
        path: target_dir.join(&name),
        name,
    })
}

fn archive_entry_root(entry_path: &Path) -> Result<String, crate::Error> {
    let mut components = entry_path.components();
    let root_component = components
        .next()
        .ok_or_else(|| crate::Error::InvalidUpdateArchive("empty archive entry".into()))?;

    match root_component {
        Component::Normal(component) => Ok(component.to_string_lossy().to_string()),
        _ => Err(crate::Error::InvalidUpdateArchive(
            "archive entry has invalid root".into(),
        )),
    }
}

fn validate_bundle_root(
    root_name: &mut Option<String>,
    entry_root: String,
) -> Result<(), crate::Error> {
    match root_name {
        Some(existing) if existing != &entry_root => Err(crate::Error::InvalidUpdateArchive(
            "archive contains multiple bundle roots".into(),
        )),
        Some(_) => Ok(()),
        None => {
            if !entry_root.ends_with(".app") {
                return Err(crate::Error::InvalidUpdateArchive(
                    "archive root must be an .app bundle".into(),
                ));
            }
            *root_name = Some(entry_root);
            Ok(())
        }
    }
}

fn shell_quote(path: &Path) -> String {
    let path = path.display().to_string().replace('\'', "'\"'\"'");
    format!("'{path}'")
}

fn do_shell_script_with_privileges(shell_script: &str) -> String {
    let escaped = shell_script.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "'do shell script \"{}\" with administrator privileges'",
        escaped
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gzip_tar(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);

        for (path, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o644);
            header.set_size(contents.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, path, Cursor::new(*contents))
                .unwrap();
        }

        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn extracts_bundle_root_name() {
        let archive = gzip_tar(&[("Char.app/Contents/Info.plist", b"plist")]);
        let dir = tempfile::tempdir().unwrap();

        let bundle = extract_macos_bundle(&archive, dir.path()).unwrap();

        assert_eq!(bundle.name, "Char.app");
        assert_eq!(bundle.path, dir.path().join("Char.app"));
        assert!(dir.path().join("Char.app/Contents/Info.plist").exists());
    }

    #[test]
    fn rejects_multiple_bundle_roots() {
        let archive = gzip_tar(&[
            ("Char.app/Contents/Info.plist", b"plist"),
            ("Other.app/Contents/Info.plist", b"plist"),
        ]);
        let dir = tempfile::tempdir().unwrap();

        let error = extract_macos_bundle(&archive, dir.path()).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("archive contains multiple bundle roots")
        );
    }

    #[test]
    fn rejects_archive_entries_without_app_bundle_root() {
        let archive = gzip_tar(&[("Char/Contents/Info.plist", b"plist")]);
        let dir = tempfile::tempdir().unwrap();

        let error = extract_macos_bundle(&archive, dir.path()).unwrap_err();

        assert!(error.to_string().contains("root must be an .app bundle"));
    }

    #[test]
    fn install_target_uses_migration_target_name_for_known_bundle_renames() {
        let cases = [
            ("/Applications/Hyprnote.app", "Hyprnote.app", "Char.app"),
            (
                "/Applications/Hyprnote Nightly.app",
                "Hyprnote Nightly.app",
                "Char Nightly.app",
            ),
            (
                "/Applications/Hyprnote Staging.app",
                "Hyprnote Staging.app",
                "Char Staging.app",
            ),
        ];

        for (current, extracted_name, expected_target_name) in cases {
            let target = install_target_path(
                Path::new(current),
                target_bundle_name(Path::new(current), extracted_name),
            )
            .unwrap();

            assert_eq!(
                target,
                PathBuf::from("/Applications").join(expected_target_name)
            );
        }
    }

    #[test]
    fn install_target_falls_back_to_extracted_bundle_name_for_non_migration_cases() {
        let current = Path::new("/Applications/Char Nightly.app");

        let target =
            install_target_path(current, target_bundle_name(current, "Char Nightly.app")).unwrap();

        assert_eq!(target, PathBuf::from("/Applications/Char Nightly.app"));
    }

    #[test]
    fn staging_update_uses_stage_dir_for_backup_and_bundle_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let current_app = temp_dir.path().join("Applications/Hyprnote.app");
        let stage_dir = temp_dir.path().join("stage");
        fs::create_dir_all(current_app.join("Contents/MacOS")).unwrap();

        let staged_update = stage_macos_update_for_current_app(
            &gzip_tar(&[("Char.app/Contents/Info.plist", b"plist")]),
            current_app.clone(),
            &stage_dir,
        )
        .unwrap();

        assert_eq!(staged_update.current_app_path, current_app);
        assert_eq!(
            staged_update.staged_app_path,
            stage_dir.join("staged/Char.app")
        );
        assert_eq!(
            staged_update.target_app_path,
            temp_dir.path().join("Applications/Char.app")
        );
        assert_eq!(
            staged_update.current_backup_path,
            stage_dir.join("backup/Hyprnote.app")
        );
        assert_eq!(staged_update.stage_dir, stage_dir);
        assert!(staged_update.staged_app_path.exists());
    }

    #[test]
    fn staging_update_renames_nightly_even_when_archive_bundle_name_is_legacy() {
        let temp_dir = tempfile::tempdir().unwrap();
        let current_app = temp_dir.path().join("Applications/Hyprnote Nightly.app");
        let stage_dir = temp_dir.path().join("stage");
        fs::create_dir_all(current_app.join("Contents/MacOS")).unwrap();

        let staged_update = stage_macos_update_for_current_app(
            &gzip_tar(&[("Hyprnote Nightly.app/Contents/Info.plist", b"plist")]),
            current_app.clone(),
            &stage_dir,
        )
        .unwrap();

        assert_eq!(staged_update.current_app_path, current_app);
        assert_eq!(
            staged_update.staged_app_path,
            stage_dir.join("staged/Hyprnote Nightly.app")
        );
        assert_eq!(
            staged_update.target_app_path,
            temp_dir.path().join("Applications/Char Nightly.app")
        );
        assert_eq!(
            staged_update.current_backup_path,
            stage_dir.join("backup/Hyprnote Nightly.app")
        );
    }

    #[test]
    fn current_bundle_path_from_executable_uses_bundle_root() {
        let executable = Path::new("/Applications/Char.app/Contents/MacOS/char");

        let bundle = current_app_bundle_path_from_executable(executable).unwrap();

        assert_eq!(bundle, PathBuf::from("/Applications/Char.app"));
    }

    #[test]
    fn rename_conflict_is_rejected() {
        let temp_dir = tempfile::tempdir().unwrap();
        let current = temp_dir.path().join("Hyprnote.app");
        let target = temp_dir.path().join("Char.app");
        fs::create_dir(&target).unwrap();
        let paths = MacInstallPaths {
            staged_app_path: temp_dir.path().join("Char.app"),
            current_backup_path: temp_dir.path().join("Hyprnote.app"),
            current_app_path: current,
            target_app_path: target.clone(),
        };

        let error = ensure_target_app_path_available(&paths).unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("macOS target bundle already exists at {}", target.display())
        );
    }

    #[test]
    fn authorization_script_for_renamed_bundle_fails_on_existing_target() {
        let paths = MacInstallPaths {
            staged_app_path: PathBuf::from("/tmp/tauri_updated_app/Char.app"),
            current_backup_path: PathBuf::from("/tmp/tauri_current_app/Hyprnote.app"),
            current_app_path: PathBuf::from("/Applications/Hyprnote.app"),
            target_app_path: PathBuf::from("/Applications/Char.app"),
        };

        let script = authorization_script(&paths);

        assert!(script.contains(
            "if [ -e '/Applications/Char.app' ]; then echo 'target bundle already exists' >&2; exit 1; fi;"
        ));
        assert!(!script.contains("target_backup"));
    }

    #[test]
    fn apply_command_waits_for_pid_before_swapping_and_opening_bundle() {
        let staged_update = StagedMacosUpdate {
            current_app_path: PathBuf::from("/Applications/Hyprnote.app"),
            staged_app_path: PathBuf::from("/tmp/stage/staged/Char.app"),
            target_app_path: PathBuf::from("/Applications/Char.app"),
            current_backup_path: PathBuf::from("/tmp/stage/backup/Hyprnote.app"),
            stage_dir: PathBuf::from("/tmp/stage"),
        };
        let command = build_macos_update_apply_command(4242, &staged_update);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(command.get_program(), "/bin/sh");
        assert_eq!(args[0], "-c");
        assert!(args[1].contains(r#"while kill -0 "$1" 2>/dev/null; do sleep 0.1; done;"#));
        assert!(args[1].contains("apply_update() {"));
        assert!(args[1].contains("osascript -e 'do shell script"));
        assert!(args[1].contains("with administrator privileges'"));
        assert!(args[1].contains("open -n '/Applications/Char.app'"));
        assert!(args[1].contains("rm -rf '/tmp/stage'"));
        assert_eq!(&args[2..], ["sh", "4242"]);
    }
}
