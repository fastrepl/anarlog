use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

use super::port::BrowserMeetingPort;
use super::protocol::{
    EndpointRecord, ExtensionEvent, ExtensionMessage, Hello, HelloReply, MAX_FRAME_BYTES,
    PROTOCOL_VERSION, encode_frame,
};
use super::relay::Relay;
use super::route::{BrowserMeetingRoute, Peer};

/// The directory under the user's data directory (`dirs::data_dir()`) where running apps put their
/// endpoint records and sockets. The native host looks here; it is not any app's own data folder.
pub const ENDPOINT_DIR_NAME: &str = "anarlog-browser-meeting";
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// How often a linked tab hears about changes made in Anarlog's window.
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// macOS `sun_path` holds 104 bytes including the terminator.
const MAX_SOCKET_PATH: usize = 103;
/// How long [`Endpoint::close`] waits for open connections to let go of their tabs.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a closing connection tries to tell its tab how it was let go. A host that stopped
/// reading is not waited for.
const FAREWELL_TIMEOUT: Duration = Duration::from_secs(1);

/// The endpoint's closing signal, held by every connection it accepted: set or dropped, it ends
/// them all. A connection that outlived its endpoint would otherwise keep acting for a bridge the
/// person turned off.
pub type Closing = watch::Receiver<bool>;

async fn closed(closing: &mut Closing) {
    // A dropped sender closes too.
    let _ = closing.wait_for(|closed| *closed).await;
}

fn is_closed(closing: &Closing) -> bool {
    *closing.borrow() || closing.has_changed().is_err()
}

/// What a connection must present: this launch's secret and an extension origin this app's native
/// host registration allows.
#[derive(Clone)]
pub struct Admission {
    pub secret: String,
    pub allowed_origins: Vec<String>,
}

impl Admission {
    fn admit(&self, hello: &Hello) -> Result<Peer, &'static str> {
        if hello.protocol != PROTOCOL_VERSION {
            return Err("Update Anarlog and the extension together.");
        }
        if !constant_time_eq(hello.secret.as_bytes(), self.secret.as_bytes())
            || !self.allowed_origins.contains(&hello.origin)
        {
            return Err("Open Anarlog, then click the extension again.");
        }
        Ok(Peer {
            origin: hello.origin.clone(),
            browser_pid: hello.browser_pid,
        })
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn read_frame(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    Ok(Some(body))
}

async fn write_json(
    writer: &mut (impl AsyncWrite + Unpin),
    value: &impl serde::Serialize,
) -> io::Result<()> {
    writer.write_all(&encode_frame(value)?).await?;
    writer.flush().await
}

/// Writes a frame unless the endpoint closes first. `Ok(false)`: closing cut it short, so part of
/// the frame may be out and nothing more can follow it.
async fn send(
    writer: &mut (impl AsyncWrite + Unpin),
    closing: &mut Closing,
    value: &impl serde::Serialize,
) -> io::Result<bool> {
    tokio::select! {
        biased;
        () = closed(closing) => Ok(false),
        written = write_json(writer, value) => written.map(|()| true),
    }
}

/// Serves one native-host connection: the hello, then the tab's messages through a [`Relay`] until
/// either side closes or `closing` fires. However it ends, the tab is let go the way leaving does
/// (a recording it started stops, one it joined keeps going), it is told so when it can still
/// hear, and the socket is shut. Nothing waits on a host that stopped reading once `closing`
/// fires, so the connection ends even then.
pub async fn serve_connection<S, P>(
    stream: S,
    route: Arc<BrowserMeetingRoute<P>>,
    admission: &Admission,
    mut closing: Closing,
) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
    P: BrowserMeetingPort,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let hello = tokio::select! {
        hello = tokio::time::timeout(HELLO_TIMEOUT, read_frame(&mut reader)) => hello,
        () = closed(&mut closing) => return Ok(()),
    };
    let hello = hello.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "no hello"))??;
    let hello = hello.and_then(|body| serde_json::from_slice::<Hello>(&body).ok());
    let peer = match hello.as_ref().map(|hello| admission.admit(hello)) {
        Some(Ok(peer)) => peer,
        refused => {
            let message = match refused {
                Some(Err(message)) => message,
                _ => "Update Anarlog and the extension together.",
            };
            let reply = HelloReply::Refused {
                message: message.into(),
            };
            return send(&mut writer, &mut closing, &reply).await.map(drop);
        }
    };
    let welcome = HelloReply::Welcome {
        protocol: PROTOCOL_VERSION,
    };
    if !send(&mut writer, &mut closing, &welcome).await? {
        return Ok(());
    }

    // Frames are read on their own task so a poll never cuts a frame in half.
    let (frames, mut incoming) = mpsc::channel::<Vec<u8>>(64);
    let reading = tokio::spawn(async move {
        while let Ok(Some(frame)) = read_frame(&mut reader).await {
            if frames.send(frame).await.is_err() {
                break;
            }
        }
    });

    let mut relay = Relay::new(peer);
    let mut poll = tokio::time::interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // A frame closing cut short: the host can't read anything after it.
    let mut torn = false;
    let result = 'serving: loop {
        let events: Vec<ExtensionEvent> = tokio::select! {
            biased;
            () = closed(&mut closing) => break Ok(()),
            frame = incoming.recv() => match frame {
                Some(frame) => match serde_json::from_slice::<ExtensionMessage>(&frame) {
                    // Closing cuts a start short: it is cancelled, and its recording, once begun,
                    // stops (`BrowserMeetingPort::start`). A notice or camera claim cut mid-way
                    // is never handed out, so nothing posts twice.
                    Ok(message @ ExtensionMessage::MeetingStart { .. }) => tokio::select! {
                        biased;
                        () = closed(&mut closing) => break Ok(()),
                        events = relay.handle(&route, message) => events,
                    },
                    // A pause, resume, stop or detach finishes first, so the tab is let go from
                    // the state it left: a pause the recorder still acknowledges would otherwise
                    // land after the tab let go of an app recording, and nothing would resume it.
                    Ok(message) => relay.handle(&route, message).await,
                    Err(_) => vec![],
                },
                None => break Ok(()),
            },
            _ = poll.tick() => relay.poll(&route),
        };
        if is_closed(&closing) {
            break Ok(());
        }
        for event in &events {
            match send(&mut writer, &mut closing, event).await {
                Ok(true) => {}
                Ok(false) => {
                    torn = true;
                    break 'serving Ok(());
                }
                Err(error) => break 'serving Err(error),
            }
        }
    };
    reading.abort();
    // The tab is let go by owner before anything is said to it: that needs nothing from the host.
    let farewell = relay.close(&route).await;
    // It hears how it was let go if it still can; a host that hung up or stopped reading doesn't.
    if !torn {
        let _ = tokio::time::timeout(FAREWELL_TIMEOUT, async {
            for event in &farewell {
                write_json(&mut writer, event).await?;
            }
            writer.shutdown().await
        })
        .await;
    }
    result
}

/// A running endpoint: the socket, the record the native host reads, and every connection it
/// accepted. Dropping it stops accepting, removes its socket and, while it is still this
/// endpoint's, its record, and closes those connections, each letting go of its tab by owner.
/// [`Endpoint::close`] does the same and waits for them.
#[cfg(unix)]
pub struct Endpoint {
    record_path: PathBuf,
    socket_path: PathBuf,
    accepting: tokio::task::JoinHandle<()>,
    closing: watch::Sender<bool>,
    /// Every accepted connection holds a sender; all of them gone means all have closed.
    open: mpsc::Receiver<()>,
    stopped: bool,
}

#[cfg(unix)]
impl Endpoint {
    fn stop(&mut self) {
        self.accepting.abort();
        self.closing.send_replace(true);
        if std::mem::replace(&mut self.stopped, true) {
            return;
        }
        if let Err(error) = remove_own_record(&self.record_path, &self.socket_path) {
            tracing::warn!(%error, "browser_meeting_record_remove_failed");
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }

    /// Closes the endpoint and waits, briefly, until every connection let go of its tab.
    /// Returns whether they all did in time.
    pub async fn close(mut self) -> bool {
        self.stop();
        tokio::time::timeout(CLOSE_TIMEOUT, self.open.recv())
            .await
            .is_ok()
    }
}

#[cfg(unix)]
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Holds `<record>.lock` while replacing or removing an app's record, so of this app's endpoints
/// and launches, one removes the record only while it is still the one it wrote. The native host
/// reads only `*.json` and never needs it: records are replaced by rename.
#[cfg(unix)]
fn lock_record(record_path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(record_path.with_extension("lock"))?;
    lock.lock()?;
    Ok(lock)
}

/// A newer endpoint for the same app (the bridge turned on again, or another launch) replaced the
/// record in place; its record stays.
#[cfg(unix)]
fn remove_own_record(record_path: &Path, socket_path: &Path) -> io::Result<()> {
    let _lock = lock_record(record_path)?;
    let bytes = match std::fs::read(record_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let ours = serde_json::from_slice::<EndpointRecord>(&bytes)
        .is_ok_and(|record| Path::new(&record.socket) == socket_path);
    if ours {
        std::fs::remove_file(record_path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let metadata = std::fs::symlink_metadata(dir)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "endpoint directory is not a directory",
        ));
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(unix)]
fn write_record(path: &Path, record: &EndpointRecord) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let _lock = lock_record(path)?;
    let temporary = path.with_extension(format!("json.{}", uuid::Uuid::new_v4().simple()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(record)?)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)
}

/// Starts serving the browser bridge for this app launch: a 0600 Unix socket and its endpoint
/// record (`<app_identifier>.json`) in `dir`, a 0700 directory. `allowed_origins` are the
/// `chrome-extension://<id>/` origins this app's native host manifest allows. Only one record per
/// app identifier exists; a newer launch replaces it.
#[cfg(unix)]
pub fn serve<P: BrowserMeetingPort>(
    dir: &Path,
    app_identifier: &str,
    allowed_origins: Vec<String>,
    route: Arc<BrowserMeetingRoute<P>>,
) -> io::Result<Endpoint> {
    use std::os::unix::fs::PermissionsExt;
    if app_identifier.is_empty()
        || !app_identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid app identifier",
        ));
    }
    private_dir(dir)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let socket_path = dir.join(format!("s-{}.sock", &id[..8]));
    if socket_path.as_os_str().len() > MAX_SOCKET_PATH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "endpoint socket path is too long",
        ));
    }
    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    let admission = Admission {
        secret: format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ),
        allowed_origins,
    };
    let record = EndpointRecord {
        protocol: PROTOCOL_VERSION,
        app: app_identifier.to_owned(),
        pid: std::process::id(),
        socket: socket_path.to_string_lossy().into_owned(),
        secret: admission.secret.clone(),
        allowed_origins: admission.allowed_origins.clone(),
    };
    let record_path = dir.join(format!("{app_identifier}.json"));
    if let Err(error) = write_record(&record_path, &record) {
        let _ = std::fs::remove_file(&socket_path);
        return Err(error);
    }
    let (closing, closing_rx) = watch::channel(false);
    let (open_tx, open) = mpsc::channel::<()>(1);
    let accepting = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                // Out of descriptors or similar: back off instead of spinning.
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            };
            let route = route.clone();
            let admission = admission.clone();
            let closing = closing_rx.clone();
            let open = open_tx.clone();
            tokio::spawn(async move {
                let _ = serve_connection(stream, route, &admission, closing).await;
                drop(open);
            });
        }
    });
    Ok(Endpoint {
        record_path,
        socket_path,
        accepting,
        closing,
        open,
        stopped: false,
    })
}
