//! What reaches past GPUI to the OS: system file dialogs and alerts, files, folders and links
//! opened in other apps, permission prompts and completion sounds. The screens harness renders
//! without a window and calls [`go_headless`] first, so no capture can show a system dialog,
//! launch an app or play a sound, whatever it clicks. GPUI's own calls (`open_url`,
//! `prompt_for_paths`, `reveal_path`, ...) already go to its test platform there.

use std::{
    ffi::OsStr,
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
};

static HEADLESS: AtomicBool = AtomicBool::new(false);

/// Turns every dialog, launch, prompt and sound above into a no-op for the rest of the process.
pub fn go_headless() {
    HEADLESS.store(true, Ordering::Relaxed);
}

pub fn headless() -> bool {
    HEADLESS.load(Ordering::Relaxed)
}

/// Opens a file, folder or URL in its default app.
pub fn open(target: impl AsRef<OsStr>) -> std::io::Result<()> {
    if headless() {
        return Ok(());
    }
    open::that(target)
}

/// Shows an `rfd` dialog; headless, it answers as if cancelled without showing anything.
pub fn dialog<T, F: Future<Output = Option<T>>>(
    show: impl FnOnce() -> F,
) -> impl Future<Output = Option<T>> {
    let shown = (!headless()).then(show);
    async move {
        match shown {
            Some(shown) => shown.await,
            None => None,
        }
    }
}

/// Like the shipping app, selects the file in the file manager, else opens its folder.
#[cfg(target_os = "linux")]
pub fn reveal_path(path: &std::path::Path) {
    if headless() {
        return;
    }
    let path = path.to_path_buf();
    if let Err(error) = std::thread::Builder::new()
        .name("native-reveal".into())
        .spawn(move || {
            futures::executor::block_on(async move {
                let Err(error) = select_in_file_manager(&path).await else {
                    return;
                };
                tracing::debug!(%error, "could not select file through the file manager");
                let result = async {
                    use std::os::fd::AsFd;

                    let file = std::fs::File::open(&path)?;
                    ashpd::desktop::open_uri::OpenDirectoryRequest::default()
                        .send(&file.as_fd())
                        .await?
                        .response()?;
                    Ok::<_, anyhow::Error>(())
                }
                .await;
                if let Err(error) = result {
                    tracing::warn!(%error, "could not reveal file through desktop portal");
                    let directory = if path.is_dir() {
                        Some(path.as_path())
                    } else {
                        path.parent()
                    };
                    if let Some(directory) = directory
                        && let Err(error) = open(directory)
                    {
                        tracing::error!(%error, "could not open containing folder");
                    }
                }
            });
        })
    {
        tracing::error!(%error, "could not start native reveal thread");
    }
}

#[cfg(target_os = "linux")]
async fn select_in_file_manager(path: &std::path::Path) -> anyhow::Result<()> {
    let uri = ashpd::url::Url::from_file_path(path)
        .map_err(|()| anyhow::anyhow!("{} is not an absolute path", path.display()))?;
    ashpd::zbus::Connection::session()
        .await?
        .call_method(
            Some("org.freedesktop.FileManager1"),
            "/org/freedesktop/FileManager1",
            Some("org.freedesktop.FileManager1"),
            "ShowItems",
            &(vec![uri.as_str()], ""),
        )
        .await?;
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn reveal_outside_tokio_selects_the_file_before_opening_its_parent() {
        use std::{
            io::BufRead,
            os::unix::fs::PermissionsExt,
            path::{Path, PathBuf},
            process::{Command, Stdio},
            time::Duration,
        };

        const FILE: &str = "ANLG_REVEAL_TEST_FILE";
        const RESULT: &str = "ANLG_REVEAL_TEST_RESULT";
        const EXPECTED: &str = "ANLG_REVEAL_TEST_EXPECTED";
        const FILE_MANAGER: &str = "ANLG_REVEAL_TEST_FILE_MANAGER";
        // Isolate D-Bus and opener environment from parallel tests and the user's desktop.
        if let Some(file) = std::env::var_os(FILE) {
            let result = PathBuf::from(std::env::var_os(RESULT).unwrap());
            let expected = std::env::var(EXPECTED).unwrap();
            if std::env::var_os(FILE_MANAGER).is_some() {
                serve_file_manager(result.clone());
            }
            assert!(tokio::runtime::Handle::try_current().is_err());
            super::reveal_path(Path::new(&file));
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while std::fs::read_to_string(&result).ok().as_deref() != Some(expected.as_str()) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "reveal did not reach {expected}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("export.txt");
        std::fs::write(&file, "exported note").unwrap();
        let opener = directory.path().join("xdg-open");
        std::fs::write(
            &opener,
            "#!/bin/sh\nprintf '%s' \"$1\" > \"$ANLG_REVEAL_TEST_RESULT\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&opener, std::fs::Permissions::from_mode(0o700)).unwrap();
        let reveal = |bus: &str, expected: &str, file_manager: bool| {
            let result = directory.path().join("revealed");
            let _ = std::fs::remove_file(&result);
            let mut child = Command::new(std::env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "platform::os::tests::reveal_outside_tokio_selects_the_file_before_opening_its_parent",
                    "--nocapture",
                ])
                .env(FILE, &file)
                .env(RESULT, result)
                .env(EXPECTED, expected)
                .env("PATH", directory.path())
                .env("DBUS_SESSION_BUS_ADDRESS", bus);
            if file_manager {
                child.env(FILE_MANAGER, "1");
            }
            child.output().unwrap()
        };
        let succeeded = |output: std::process::Output| {
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        };

        let absent = format!(
            "unix:path={}",
            directory.path().join("absent-bus").display()
        );
        succeeded(reveal(&absent, directory.path().to_str().unwrap(), false));

        // A private bus without service activation, so only the fake file manager answers.
        let config = directory.path().join("bus.conf");
        std::fs::write(
            &config,
            format!(
                "<busconfig><type>session</type><listen>unix:path={}</listen>\
                 <policy context=\"default\"><allow send_destination=\"*\"/>\
                 <allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>",
                directory.path().join("bus").display()
            ),
        )
        .unwrap();
        let mut bus = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon runs the private test bus");
        let mut address = String::new();
        std::io::BufReader::new(bus.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let uri = ashpd::url::Url::from_file_path(&file).unwrap();
        let selected = reveal(address.trim(), uri.as_str(), true);
        bus.kill().unwrap();
        bus.wait().unwrap();
        succeeded(selected);
    }

    /// Answers `ShowItems` on the session bus by writing the URIs it was asked to select.
    fn serve_file_manager(result: std::path::PathBuf) {
        use ashpd::zbus::{Connection, MessageStream, message::Type};
        use futures::StreamExt;

        let (ready, serving) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            futures::executor::block_on(async move {
                let connection = Connection::session().await.unwrap();
                let mut calls = MessageStream::from(&connection);
                connection
                    .request_name("org.freedesktop.FileManager1")
                    .await
                    .unwrap();
                ready.send(()).unwrap();
                while let Some(Ok(call)) = calls.next().await {
                    let header = call.header();
                    if header.message_type() != Type::MethodCall
                        || header
                            .member()
                            .is_none_or(|member| member.as_str() != "ShowItems")
                    {
                        continue;
                    }
                    let (uris, _): (Vec<String>, String) = call.body().deserialize().unwrap();
                    std::fs::write(&result, uris.join("\n")).unwrap();
                    connection.reply(&header, &()).await.unwrap();
                }
            });
        });
        serving
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("fake file manager owns its bus name");
    }

    #[test]
    fn zbus_connects_outside_tokio_without_reactor_panic() {
        // Must fail cleanly with a connection error rather than panicking for a missing
        // Tokio reactor if any crate re-enables zbus's non-additive `tokio` feature.
        let directory = tempfile::tempdir().unwrap();
        let address = format!(
            "unix:path={}",
            directory.path().join("absent-bus").display()
        );
        let result = std::thread::spawn(move || {
            assert!(tokio::runtime::Handle::try_current().is_err());
            futures::executor::block_on(async move {
                ashpd::zbus::connection::Builder::address(address.as_str())
                    .unwrap()
                    .build()
                    .await
            })
        })
        .join()
        .expect("zbus connect outside Tokio must not panic");
        assert!(result.is_err());
    }
}
