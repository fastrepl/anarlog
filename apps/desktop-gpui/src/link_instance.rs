//! Windows starts a new process for each protocol activation. A profile-scoped pipe keeps
//! callbacks in memory and forwards them before a second runtime can open the database.

use std::{ffi::OsString, io, path::Path, time::Duration};

use desktop_gpui::{native_events::NativeEvents, platform::deeplinks::SCHEMES};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
    runtime::{Builder, Runtime},
    time::{sleep, timeout},
};

const MAX_URL_BYTES: usize = 64 * 1024;
const FORWARD_TIMEOUT: Duration = Duration::from_secs(4);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(1);
const ERROR_PIPE_BUSY: i32 = 231;

pub fn parse_argument(arg: OsString) -> anyhow::Result<String> {
    arg.into_string()
        .ok()
        .filter(|url| {
            url.len() <= MAX_URL_BYTES
                && url
                    .split_once(':')
                    .is_some_and(|(scheme, _)| SCHEMES.contains(&scheme))
        })
        .ok_or_else(|| anyhow::anyhow!("Expected an Anarlog protocol URL. Use --help."))
}

/// Hold the returned runtime open for the lifetime of the primary app. A secondary launch
/// returns `None` after the running app has accepted the callback into its inbox.
pub fn start(
    profile: &Path,
    url: Option<String>,
    events: NativeEvents,
) -> anyhow::Result<Option<Runtime>> {
    std::fs::create_dir_all(profile)?;
    let path = profile.canonicalize()?;
    let digest: String = Sha256::digest(path.as_os_str().as_encoded_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let name = format!(r"\\.\pipe\anarlog-gpui-{digest}");
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let Some(server) = runtime.block_on(timeout_start(&name, url.as_deref()))? else {
        return Ok(None);
    };
    if let Some(url) = url {
        events.receive_deep_link(url)?;
    }
    runtime.spawn(async move {
        if let Err(error) = serve(&name, server, events).await {
            tracing::warn!(%error, "Windows deep-link listener stopped");
        }
    });
    Ok(Some(runtime))
}

async fn timeout_start(name: &str, url: Option<&str>) -> anyhow::Result<Option<NamedPipeServer>> {
    timeout(FORWARD_TIMEOUT, async {
        loop {
            // Keep the first instance open for the lifetime of the app so only one process
            // can claim the pipe, even when launches race or the profile was opened via an alias.
            match ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(name)
            {
                Ok(server) => return Ok(Some(server)),
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {}
                Err(error) => return Err(error.into()),
            }
            let Some(url) = url else {
                anyhow::bail!("Profile is already open in another process.");
            };
            match ClientOptions::new().open(name) {
                Ok(mut client) => {
                    client.write_u32(url.len() as u32).await?;
                    client.write_all(url.as_bytes()).await?;
                    anyhow::ensure!(
                        client.read_u8().await? == 1,
                        "Running app rejected the link."
                    );
                    return Ok(None);
                }
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        || error.raw_os_error() == Some(ERROR_PIPE_BUSY) =>
                {
                    sleep(Duration::from_millis(25)).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("Timed out delivering the link to the running app."))?
}

async fn serve(name: &str, mut listener: NamedPipeServer, events: NativeEvents) -> io::Result<()> {
    loop {
        listener.connect().await?;
        // Always keep an instance of the pipe open. Use a fresh listener so overlapped
        // I/O from the previous connection cannot affect the next one.
        let next = ServerOptions::new()
            .reject_remote_clients(true)
            .create(name)?;
        let mut connection = std::mem::replace(&mut listener, next);
        // Bound how long an incomplete request can occupy the pipe. Once a complete
        // request is read, keep the connection open until the client has read the reply.
        let _ = timeout(CLIENT_TIMEOUT, async {
            let len = connection.read_u32().await? as usize;
            let accepted = if (1..=MAX_URL_BYTES).contains(&len) {
                let mut bytes = vec![0; len];
                connection.read_exact(&mut bytes).await?;
                String::from_utf8(bytes).is_ok_and(|url| events.receive_deep_link(url).is_ok())
            } else {
                false
            };
            connection.write_u8(u8::from(accepted)).await?;
            let _ = connection.read_u8().await;
            Ok::<_, io::Error>(())
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use desktop_gpui::native_events::NativeEvent;

    use super::*;

    #[test]
    fn accepts_the_cold_launch_link_then_forwards_later_links() {
        let profile = tempfile::tempdir().unwrap();
        let events = NativeEvents::default();
        let cold = "anarlog://auth/callback?code=test-code&state=test-state";
        let primary = start(
            profile.path(),
            Some(parse_argument(cold.into()).unwrap()),
            events.clone(),
        )
        .unwrap()
        .unwrap();
        assert!(matches!(events.next(), Some(NativeEvent::DeepLink { raw, .. }) if raw == cold));
        for raw in [
            "anarlog://onboarding-demo/complete",
            "anarlog://billing/refresh",
        ] {
            let url = parse_argument(raw.into()).unwrap();
            assert!(
                start(profile.path(), Some(url), NativeEvents::default())
                    .unwrap()
                    .is_none()
            );
            assert!(
                matches!(events.next(), Some(NativeEvent::DeepLink { raw: delivered, .. }) if delivered == raw)
            );
        }
        drop(primary);
        assert!(start(profile.path(), None, events).unwrap().is_some());
    }
}
