use crate::{
    CdpError, CdpPage, ChromiumLaunchConfig, ChromiumLaunchError, ChromiumProcess, GoogleMeetUrl,
};

pub struct MeetingSession {
    browser: Option<ChromiumProcess>,
    page: Option<CdpPage>,
}

impl MeetingSession {
    pub async fn launch(
        config: ChromiumLaunchConfig,
        meeting_url: &GoogleMeetUrl,
    ) -> Result<Self, MeetingSessionLaunchError> {
        let browser = ChromiumProcess::launch(config).await?;
        match CdpPage::open(&browser.devtools_websocket_url, meeting_url).await {
            Ok(page) => Ok(Self {
                browser: Some(browser),
                page: Some(page),
            }),
            Err(source) => {
                let cleanup_error = browser.shutdown().await.err();
                Err(MeetingSessionLaunchError::OpenPage {
                    source,
                    cleanup_error,
                })
            }
        }
    }

    pub fn page_mut(&mut self) -> &mut CdpPage {
        self.page.as_mut().expect("active session owns a page")
    }

    pub fn browser_id(&self) -> Option<u32> {
        self.browser.as_ref().and_then(ChromiumProcess::id)
    }

    pub async fn shutdown(mut self) -> Result<(), MeetingSessionShutdownError> {
        let page_error = match self.page.take() {
            Some(page) => page.close().await.err(),
            None => None,
        };
        let browser_error = match self.browser.take() {
            Some(browser) => browser.shutdown().await.err(),
            None => None,
        };
        if page_error.is_none() && browser_error.is_none() {
            Ok(())
        } else {
            Err(MeetingSessionShutdownError {
                page_error,
                browser_error,
            })
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MeetingSessionLaunchError {
    #[error(transparent)]
    Chromium(#[from] ChromiumLaunchError),
    #[error("failed to open the Google Meet page (Chromium cleanup error: {cleanup_error:?})")]
    OpenPage {
        #[source]
        source: CdpError,
        cleanup_error: Option<std::io::Error>,
    },
}

#[derive(Debug)]
pub struct MeetingSessionShutdownError {
    pub page_error: Option<CdpError>,
    pub browser_error: Option<std::io::Error>,
}

impl std::fmt::Display for MeetingSessionShutdownError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "meeting session cleanup failed (page: {:?}, Chromium: {:?})",
            self.page_error, self.browser_error
        )
    }
}

impl std::error::Error for MeetingSessionShutdownError {}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{path::Path, time::Duration};

    use futures_util::StreamExt;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    fn write_fake_chromium(path: &Path, port: u16) {
        std::fs::write(
            path,
            format!(
                r#"#!/bin/sh
profile=""
for arg in "$@"; do
  case "$arg" in
    --user-data-dir=*) profile="${{arg#--user-data-dir=}}" ;;
  esac
done
printf '{port}\n/devtools/browser/test-id\n' > "$profile/DevToolsActivePort"
while true; do sleep 1; done
"#
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn owns_the_page_and_browser_cleanup_as_one_session() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut discovery, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let read = discovery.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("PUT /json/new?https%3A%2F%2Fmeet.google.com"));
            let body =
                format!(r#"{{"webSocketDebuggerUrl":"ws://{address}/devtools/page/test-id"}}"#);
            discovery
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            discovery.shutdown().await.unwrap();

            let (page, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(page).await.unwrap();
            assert!(matches!(
                websocket.next().await,
                Some(Ok(Message::Close(_)))
            ));
        });

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("chromium");
        write_fake_chromium(&executable, address.port());
        let meeting = GoogleMeetUrl::parse("https://meet.google.com/abc-defg-hij").unwrap();
        let session = MeetingSession::launch(
            ChromiumLaunchConfig {
                binary: executable,
                user_data_dir: directory.path().join("profile"),
                locale: "en-US".into(),
                authenticated: false,
                headless: true,
                disable_sandbox: false,
                startup_timeout: Duration::from_secs(2),
            },
            &meeting,
        )
        .await
        .unwrap();

        assert!(session.browser_id().is_some());
        session.shutdown().await.unwrap();
        server.await.unwrap();
    }
}
