//! Linux and Windows open a protocol link by starting the app with the URL as an argument, as
//! they do for every launch. Like shipping's single-instance plugin, a later launch hands its
//! links to the instance already serving this profile, which shows its main window, and exits
//! before opening the database that instance holds. Each profile, and so each channel, has its
//! own instance.

use std::{io, path::Path, time::Duration};

use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    runtime::Runtime,
    time::timeout,
};

const MAX_REQUEST_BYTES: u32 = 256 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);

/// Serves this profile's later launches until dropped.
pub struct Instance {
    _runtime: Runtime,
}

/// Serves later launches of the profile at `profile`, passing their links (none for a plain
/// launch) to `open`, which says whether it took them. When another instance already serves the
/// profile, hands it `urls` instead and returns `None`.
pub fn claim(
    profile: &Path,
    urls: &[String],
    open: impl Fn(Vec<String>) -> bool + Send + Sync + 'static,
) -> io::Result<Option<Instance>> {
    // Windows canonicalizes an existing folder to an extended-length path only, so the first
    // launch of a profile creates it to name the same channel as every later one.
    std::fs::create_dir_all(profile)?;
    let profile = std::fs::canonicalize(profile)?;
    let digest: String = Sha256::digest(profile.as_os_str().as_encoded_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("single-instance")
        .enable_all()
        .build()?;
    let Some(listener) = runtime.block_on(transport::claim(&digest, urls))? else {
        return Ok(None);
    };
    runtime.spawn(serve(listener, open));
    Ok(Some(Instance { _runtime: runtime }))
}

async fn forward(
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    urls: &[String],
) -> io::Result<()> {
    let request = serde_json::to_vec(urls)?;
    let answer = timeout(TIMEOUT, async {
        stream.write_u32(request.len() as u32).await?;
        stream.write_all(&request).await?;
        stream.flush().await?;
        stream.read_u8().await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "The running app did not answer"))??;
    match answer {
        1 => Ok(()),
        _ => Err(io::Error::other("The running app refused the launch")),
    }
}

async fn serve(mut listener: transport::Listener, open: impl Fn(Vec<String>) -> bool) {
    loop {
        let mut stream = match listener.accept().await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "could not accept a later launch");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let served = timeout(TIMEOUT, async {
            let length = stream.read_u32().await?;
            if length > MAX_REQUEST_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "launch too long",
                ));
            }
            let mut request = vec![0; length as usize];
            stream.read_exact(&mut request).await?;
            let taken = serde_json::from_slice(&request).is_ok_and(&open);
            stream.write_u8(u8::from(taken)).await?;
            stream.flush().await?;
            // Closing a Windows pipe discards what the launch hasn't read; wait for it to close.
            let _ = stream.read_u8().await;
            Ok(())
        })
        .await;
        match served {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!(%error, "could not serve a later launch"),
            Err(_) => tracing::warn!("a later launch timed out"),
        }
    }
}

/// An abstract socket per user and profile: it disappears with its process, so no stale file
/// can block a launch. Both ends check the other is this user, since a sign-in callback carries
/// credentials and any local user can reach the abstract namespace.
#[cfg(target_os = "linux")]
mod transport {
    use std::{
        io,
        os::{
            linux::net::SocketAddrExt,
            unix::net::{SocketAddr, UnixListener as StdUnixListener, UnixStream as StdUnixStream},
        },
    };

    use tokio::net::{UnixListener, UnixStream};

    pub struct Listener {
        socket: UnixListener,
        user: u32,
    }

    pub async fn claim(digest: &str, urls: &[String]) -> io::Result<Option<Listener>> {
        let user = nix::unistd::geteuid().as_raw();
        let address = SocketAddr::from_abstract_name(format!("anarlog-gpui-{user}-{digest}"))?;
        match StdUnixListener::bind_addr(&address) {
            Ok(socket) => {
                socket.set_nonblocking(true)?;
                Ok(Some(Listener {
                    socket: UnixListener::from_std(socket)?,
                    user,
                }))
            }
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                let stream = StdUnixStream::connect_addr(&address)?;
                stream.set_nonblocking(true)?;
                let stream = UnixStream::from_std(stream)?;
                same_user(&stream, user)?;
                super::forward(stream, urls).await?;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    impl Listener {
        pub async fn accept(&mut self) -> io::Result<UnixStream> {
            let (stream, _) = self.socket.accept().await?;
            same_user(&stream, self.user)?;
            Ok(stream)
        }
    }

    fn same_user(stream: &UnixStream, user: u32) -> io::Result<()> {
        if stream.peer_cred()?.uid() == user {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the other end belongs to another user",
            ))
        }
    }
}

/// A named pipe per profile. The first instance of the name serves; a launch that can't create
/// it connects instead, retrying while the serving instance is between connections. Any local
/// user can create the name first, so a launch checks the server is this user before handing it
/// a sign-in callback; the pipe's default access already keeps other users from writing to it.
#[cfg(target_os = "windows")]
mod transport {
    use std::{io, os::windows::io::AsRawHandle, time::Duration};

    use tokio::{
        net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
        time::{Instant, sleep},
    };

    const ERROR_PIPE_BUSY: i32 = 231;

    pub struct Listener {
        name: String,
        next: NamedPipeServer,
    }

    pub async fn claim(digest: &str, urls: &[String]) -> io::Result<Option<Listener>> {
        let name = format!(r"\\.\pipe\anarlog-gpui-{digest}");
        let deadline = Instant::now() + super::TIMEOUT;
        loop {
            match ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(&name)
            {
                Ok(next) => return Ok(Some(Listener { name, next })),
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {}
                Err(error) => return Err(error),
            }
            match ClientOptions::new().open(&name) {
                Ok(client) => {
                    same_user(&client)?;
                    super::forward(client, urls).await?;
                    return Ok(None);
                }
                Err(error)
                    if (error.kind() == io::ErrorKind::NotFound
                        || error.raw_os_error() == Some(ERROR_PIPE_BUSY))
                        && Instant::now() < deadline =>
                {
                    sleep(Duration::from_millis(25)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    impl Listener {
        pub async fn accept(&mut self) -> io::Result<NamedPipeServer> {
            let connected = self.next.connect().await;
            // Keep an instance open between launches, so a launch never finds the name free.
            let next = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&self.name)?;
            let current = std::mem::replace(&mut self.next, next);
            connected.map(|()| current)
        }
    }

    fn same_user(pipe: &impl AsRawHandle) -> io::Result<()> {
        use windows::Win32::{
            Foundation::{CloseHandle, HANDLE},
            Security::{EqualSid, GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser},
            System::{
                Pipes::GetNamedPipeServerProcessId,
                Threading::{
                    GetCurrentProcess, OpenProcess, OpenProcessToken,
                    PROCESS_QUERY_LIMITED_INFORMATION,
                },
            },
        };

        // A TOKEN_USER and the SID it points into, in words so the struct is aligned.
        fn user(process: HANDLE) -> windows::core::Result<Vec<usize>> {
            let mut token = HANDLE::default();
            unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }?;
            let mut size = 0;
            // Fails with the size it needs.
            let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut size) };
            let mut user = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
            let read = unsafe {
                GetTokenInformation(
                    token,
                    TokenUser,
                    Some(user.as_mut_ptr().cast()),
                    size,
                    &mut size,
                )
            };
            let _ = unsafe { CloseHandle(token) };
            read.map(|()| user)
        }

        let mut server = 0;
        unsafe { GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut server) }?;
        let ours = user(unsafe { GetCurrentProcess() })?;
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, server) };
        let theirs = process.and_then(|process| {
            let user = user(process);
            let _ = unsafe { CloseHandle(process) };
            user
        });
        let sid = |user: &[usize]| unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        match theirs {
            Ok(theirs) if unsafe { EqualSid(sid(&ours), sid(&theirs)) }.is_ok() => Ok(()),
            _ => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the running app belongs to another user",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        native_events::NativeEvent, native_events::NativeEvents, platform::tray::TrayAction,
    };

    use super::*;

    #[test]
    fn a_later_launch_hands_its_link_to_the_running_instance_and_exits() {
        let profile = tempfile::tempdir().unwrap();
        let events = NativeEvents::default();
        let running = {
            let events = events.clone();
            claim(profile.path(), &[], move |urls| events.relaunched(urls))
        }
        .unwrap();
        assert!(running.is_some());

        let link = "anarlog-staging://auth/callback?code=test";
        let later = claim(&profile.path().join("."), &[link.into()], |_| {
            panic!("the running instance serves the profile")
        })
        .unwrap();
        assert!(later.is_none());
        assert!(matches!(
            events.next(),
            Some(NativeEvent::Action(TrayAction::Open))
        ));
        assert!(matches!(events.next(), Some(NativeEvent::DeepLink { raw, .. }) if raw == link));

        drop(running);
        assert!(
            claim(profile.path(), &[], |_| true).unwrap().is_some(),
            "the next launch serves once the running instance exits"
        );
    }

    #[test]
    fn a_fresh_profiles_first_launch_receives_the_sign_in_callback() {
        let parent = tempfile::tempdir().unwrap();
        let profile = parent.path().join("profile");
        let events = NativeEvents::default();
        let running = {
            let events = events.clone();
            claim(&profile, &[], move |urls| events.relaunched(urls))
        }
        .unwrap();
        assert!(running.is_some());

        let link = "anarlog-staging://auth/callback?code=test";
        let callback = claim(&profile, &[link.into()], |_| {
            panic!("the first launch serves the profile it created")
        })
        .unwrap();
        assert!(callback.is_none());
        assert!(matches!(
            events.next(),
            Some(NativeEvent::Action(TrayAction::Open))
        ));
        assert!(matches!(events.next(), Some(NativeEvent::DeepLink { raw, .. }) if raw == link));
    }
}
