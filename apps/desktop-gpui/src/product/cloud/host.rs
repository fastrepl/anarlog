#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::sync::Arc;

use desktop_runtime::{Result, ServiceError, SessionId};
use futures::{
    SinkExt, StreamExt,
    channel::{mpsc, oneshot},
    future::BoxFuture,
};
use reqwest::Url;
use zeroize::Zeroizing;

use super::{CloudHost, transport::failure};

/// The card the main window shows while a page continues in the browser (shipping's
/// `InstructionType`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Handoff {
    SignIn,
    Billing,
    /// The integration's id, such as `google-calendar` or `slack`.
    Integration(String),
}

pub enum HostCommand {
    OpenUrl(Url, oneshot::Sender<Result<()>>),
    HandOff(Url, Handoff, oneshot::Sender<Result<()>>),
    Copy(Zeroizing<String>, oneshot::Sender<Result<()>>),
    Flush(SessionId, oneshot::Sender<Result<()>>),
}

/// Keep the receiver in the application, not in a transient settings pane.
pub struct NativeHost {
    sender: mpsc::Sender<HostCommand>,
}

impl NativeHost {
    pub fn install(
        cx: &mut gpui::App,
        flush: impl Fn(SessionId, &mut gpui::App) -> BoxFuture<'static, Result<()>> + 'static,
        hand_off: impl Fn(Url, Handoff, &mut gpui::App) + 'static,
    ) -> Arc<Self> {
        let (host, mut commands) = Self::channel();
        cx.spawn(async move |cx| {
            while let Some(command) = commands.next().await {
                match command {
                    HostCommand::OpenUrl(url, reply) => {
                        cx.update(|cx| cx.open_url(url.as_str()));
                        let _ = reply.send(Ok(()));
                    }
                    HostCommand::HandOff(url, handoff, reply) => {
                        cx.update(|cx| hand_off(url, handoff, cx));
                        let _ = reply.send(Ok(()));
                    }
                    HostCommand::Copy(text, reply) => {
                        cx.update(|cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                text.to_string(),
                            ));
                        });
                        let _ = reply.send(Ok(()));
                    }
                    HostCommand::Flush(session, reply) => {
                        let result = cx.update(|cx| flush(session, cx)).await;
                        let _ = reply.send(result);
                    }
                }
            }
        })
        .detach();
        host
    }

    pub fn channel() -> (Arc<Self>, mpsc::Receiver<HostCommand>) {
        let (sender, receiver) = mpsc::channel(8);
        (Arc::new(Self { sender }), receiver)
    }

    fn request(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<()>>) -> HostCommand + Send + 'static,
    ) -> BoxFuture<'static, Result<()>> {
        let mut sender = self.sender.clone();
        Box::pin(async move {
            let (reply, receive) = oneshot::channel();
            sender
                .send(make(reply))
                .await
                .map_err(|_| ServiceError::Closed)?;
            receive.await.map_err(|_| ServiceError::Closed)?
        })
    }
}

impl CloudHost for NativeHost {
    fn open_url(&self, url: Url) -> BoxFuture<'static, Result<()>> {
        self.request(move |reply| HostCommand::OpenUrl(url, reply))
    }

    fn hand_off(&self, url: Url, handoff: Handoff) -> BoxFuture<'static, Result<()>> {
        self.request(move |reply| HostCommand::HandOff(url, handoff, reply))
    }

    fn copy_text(&self, text: Zeroizing<String>) -> BoxFuture<'static, Result<()>> {
        self.request(move |reply| HostCommand::Copy(text, reply))
    }

    fn flush_editor(&self, session: SessionId) -> BoxFuture<'static, Result<()>> {
        self.request(move |reply| HostCommand::Flush(session, reply))
    }

    fn export_recovery(&self, code: Zeroizing<String>) -> BoxFuture<'static, Result<()>> {
        Box::pin(async move {
            let file = rfd::AsyncFileDialog::new()
                .set_title("Save your Anarlog recovery key securely")
                .set_file_name("anarlog-recovery-key.txt")
                .save_file()
                .await
                .ok_or(ServiceError::Cancelled)?;
            tokio::task::spawn_blocking(move || {
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    options.mode(0o600);
                }
                let mut output = options.open(file.path()).map_err(|_| {
                    failure("Choose a new recovery-key file; existing files are never overwritten")
                })?;
                std::io::Write::write_all(&mut output, code.as_bytes())
                    .map_err(|_| failure("Could not write recovery-key file"))?;
                output
                    .sync_all()
                    .map_err(|_| failure("Could not persist recovery-key file"))
            })
            .await
            .map_err(|_| failure("Recovery-key export worker stopped"))?
        })
    }
}

pub fn import_recovery() -> BoxFuture<'static, Result<Zeroizing<String>>> {
    Box::pin(async {
        let file = rfd::AsyncFileDialog::new()
            .set_title("Import Anarlog recovery key")
            .pick_file()
            .await
            .ok_or(ServiceError::Cancelled)?;
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(file.path())
                .map_err(|_| failure("Cannot open recovery-key file"))?;
            let mut contents = Zeroizing::new(String::new());
            std::io::Read::read_to_string(&mut std::io::Read::take(file, 1025), &mut contents)
                .map_err(|_| failure("Cannot read recovery-key file"))?;
            if contents.len() > 1024 {
                return Err(failure("Recovery-key file is too large"));
            }
            anlg_e2ee::RecoveryKey::parse(&contents)
                .map_err(|_| failure("Invalid recovery key"))?;
            Ok(contents)
        })
        .await
        .map_err(|_| failure("Recovery-key import worker stopped"))?
    })
}
