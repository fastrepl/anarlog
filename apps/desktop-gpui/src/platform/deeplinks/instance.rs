use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::linux::net::SocketAddrExt,
    os::unix::net::{SocketAddr, UnixListener, UnixStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use nix::{
    sys::socket::{getsockopt, sockopt::PeerCredentials},
    unistd::geteuid,
};
use sha2::{Digest, Sha256};

const MAX_URL_BYTES: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(2);

pub struct LaunchLinks {
    pending: Receiver<Option<String>>,
    stopping: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl LaunchLinks {
    /// An empty result means the existing instance accepted the launch.
    pub fn bind_or_forward(profile: &Path, link: Option<String>) -> io::Result<Option<Self>> {
        if link.as_deref().is_some_and(|url| !valid_link(url)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid protocol URL",
            ));
        }
        std::fs::create_dir_all(profile)?;
        let canonical = profile.canonicalize()?;
        // Abstract sockets avoid stale files and the Unix socket path length limit.
        let hash = Sha256::digest(canonical.as_os_str().as_encoded_bytes());
        let digest = hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let address =
            SocketAddr::from_abstract_name(format!("anarlog-gpui-{}-{digest}", geteuid()))?;
        let listener = match UnixListener::bind_addr(&address) {
            Ok(listener) => listener,
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                forward(&address, link.as_deref())?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        listener.set_nonblocking(true)?;
        let (sender, pending) = mpsc::sync_channel(64);
        if let Some(link) = link {
            enqueue(&sender, Some(link))?;
        }
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let worker = thread::Builder::new()
            .name("protocol-links".into())
            .spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if let Err(error) = receive(stream, &sender) {
                                tracing::warn!(%error, "could not receive protocol launch");
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::park_timeout(Duration::from_millis(50));
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => {
                            tracing::warn!(%error, "protocol listener stopped");
                            break;
                        }
                    }
                }
            })?;
        Ok(Some(Self {
            pending,
            stopping,
            worker: Some(worker),
        }))
    }

    pub fn pending(&self) -> impl Iterator<Item = Option<String>> + '_ {
        self.pending.try_iter()
    }
}

impl Drop for LaunchLinks {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            if worker.join().is_err() {
                tracing::warn!("protocol listener panicked");
            }
        }
    }
}

fn configure(stream: &UnixStream) -> io::Result<()> {
    // Sign-in callbacks can contain credentials; never send them to another local user.
    if getsockopt(stream, PeerCredentials)?.uid() != geteuid().as_raw() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "protocol peer is not the current user",
        ));
    }
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))
}

fn forward(address: &SocketAddr, url: Option<&str>) -> io::Result<()> {
    let mut stream = UnixStream::connect_addr(address)?;
    configure(&stream)?;
    stream.write_all(url.unwrap_or_default().as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    let mut response = [0];
    stream.read_exact(&mut response)?;
    if response == [1] {
        Ok(())
    } else {
        Err(io::Error::other(
            "the running app rejected the protocol launch",
        ))
    }
}

fn enqueue(sender: &SyncSender<Option<String>>, link: Option<String>) -> io::Result<()> {
    sender
        .try_send(link)
        .map_err(|_| io::Error::other("protocol launch queue is unavailable"))
}

fn receive(mut stream: UnixStream, sender: &SyncSender<Option<String>>) -> io::Result<()> {
    configure(&stream)?;
    let mut url = String::new();
    (&stream)
        .take((MAX_URL_BYTES + 1) as u64)
        .read_to_string(&mut url)?;
    let valid = url.is_empty() || valid_link(&url);
    let accepted = valid && enqueue(sender, (!url.is_empty()).then_some(url)).is_ok();
    stream.write_all(&[u8::from(accepted)])
}

fn valid_link(url: &str) -> bool {
    url.len() <= MAX_URL_BYTES
        && super::has_registered_scheme(url)
        && desktop_runtime::deeplink::DeepLink::parse(url).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_to_the_original_launch_until_it_exits() {
        let profile = tempfile::tempdir().unwrap();
        let auth = "anarlog://auth/callback?code=not-a-real-code";
        let share =
            "anarlog://share/open?mode=account&share_id=b95c3378-ed22-4819-b8af-b6ad3c793b86";
        let first = LaunchLinks::bind_or_forward(profile.path(), Some(auth.into()))
            .unwrap()
            .unwrap();
        assert!(
            LaunchLinks::bind_or_forward(&profile.path().join("."), Some(share.into()))
                .unwrap()
                .is_none()
        );
        assert!(
            LaunchLinks::bind_or_forward(profile.path(), None)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            first.pending().collect::<Vec<_>>(),
            [Some(auth.into()), Some(share.into()), None]
        );
        drop(first);
        assert!(
            LaunchLinks::bind_or_forward(profile.path(), None)
                .unwrap()
                .is_some()
        );
    }
}
