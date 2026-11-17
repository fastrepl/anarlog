//! Shipping's desktop update control (`main/update-banner.tsx`): available → downloading →
//! ready to restart, or failed with a retry. The sidebar toast registry renders it.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use desktop_runtime::{
    CancellationToken, ServiceError,
    updater::{NativeUpdater, ResolvedRelease, UpdateConfig, UpdateInstaller, VerifiedUpdate},
};
use gpui::Context;

use super::{ApplicationView, toasts};

/// `UPDATE_CHECK_INTERVAL_MS`.
const CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);

#[derive(Default)]
pub(super) enum Update {
    #[default]
    None,
    Checking,
    Available(Arc<NativeUpdater>, ResolvedRelease),
    Downloading(Arc<str>, Arc<Progress>),
    Ready(Arc<NativeUpdater>, Option<VerifiedUpdate>),
    Failed(Arc<NativeUpdater>, ResolvedRelease, Arc<str>),
}

#[derive(Default)]
pub(super) struct Progress {
    downloaded: AtomicU64,
    total: AtomicU64,
}

/// What the toast registry shows for the update, like shipping's `DesktopUpdateControl`.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum UpdateNotice {
    Available(Arc<str>),
    Downloading(Arc<str>, Option<f32>),
    Ready { version: Arc<str>, installing: bool },
    Failed(Arc<str>, Arc<str>),
}

impl Update {
    pub(super) fn notice(&self) -> Option<UpdateNotice> {
        Some(match self {
            Self::None | Self::Checking => return None,
            Self::Available(_, release) => UpdateNotice::Available(version(release)),
            Self::Downloading(version, progress) => {
                let total = progress.total.load(Ordering::Relaxed);
                UpdateNotice::Downloading(
                    version.clone(),
                    (total > 0).then(|| {
                        (progress.downloaded.load(Ordering::Relaxed) as f32 / total as f32)
                            .clamp(0., 1.)
                    }),
                )
            }
            Self::Ready(_, update) => UpdateNotice::Ready {
                version: update
                    .as_ref()
                    .map(|update| version(&update.release))
                    .unwrap_or_default(),
                installing: update.is_none(),
            },
            Self::Failed(_, release, message) => {
                UpdateNotice::Failed(version(release), message.clone())
            }
        })
    }

    pub(super) fn ready(&self) -> bool {
        matches!(self, Self::Ready(_, Some(_)))
    }
}

fn version(release: &ResolvedRelease) -> Arc<str> {
    release.version.to_string().into()
}

fn updater() -> desktop_runtime::Result<Arc<NativeUpdater>> {
    let endpoint = std::env::var("ANARLOG_NATIVE_UPDATE_URL").map_err(|_| {
        ServiceError::Failed("Native distribution update feed is not configured.".into())
    })?;
    let key = std::env::var("ANARLOG_NATIVE_UPDATE_PUBLIC_KEY").map_err(|_| {
        ServiceError::Failed("Native update signing public key is not configured.".into())
    })?;
    let target = std::env::var("ANARLOG_NATIVE_UPDATE_TARGET")
        .map_err(|_| ServiceError::Failed("Native update target is not configured.".into()))?;
    let url = endpoint
        .parse()
        .map_err(|_| ServiceError::Failed("Invalid native update URL".into()))?;
    let version = env!("CARGO_PKG_VERSION")
        .parse()
        .map_err(|_| ServiceError::Failed("Invalid native version".into()))?;
    NativeUpdater::new(UpdateConfig::new(vec![url], version, target, key)).map(Arc::new)
}

impl ApplicationView {
    /// The single read the toast registry makes of update state.
    pub(super) fn update_notice(&self) -> Option<UpdateNotice> {
        self.update.notice()
    }

    /// Capture is starting, live or finalizing.
    pub(super) fn meeting_live(&self) -> bool {
        self.services.as_ref().is_some_and(|services| {
            matches!(
                services.capture.take_update(u64::MAX).phase,
                crate::meeting::capture::Phase::Loading
                    | crate::meeting::capture::Phase::Listening
                    | crate::meeting::capture::Phase::Finalizing
            )
        })
    }

    /// The tray's and menu's update item: checks, downloads, or restarts into a ready update.
    pub(super) fn check_update(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        match &self.update {
            Update::Ready(..) => self.install_update(cx),
            Update::Available(..) | Update::Failed(..) => self.download_update(cx),
            Update::None => match updater() {
                Ok(updater) => self.check_feed(updater, true, cx),
                Err(error) => toasts::error(error.to_string(), cx),
            },
            Update::Checking | Update::Downloading(..) => {}
        }
    }

    /// Like shipping, a configured build checks for updates at startup and every 30 minutes.
    pub(super) fn watch_updates(&mut self, cx: &mut Context<Self>) {
        let Ok(updater) = updater() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            loop {
                let checking = this.update(cx, |this, cx| {
                    if matches!(this.update, Update::None) && !this.closing {
                        this.check_feed(updater.clone(), false, cx);
                    }
                });
                if checking.is_err() {
                    break;
                }
                cx.background_executor().timer(CHECK_INTERVAL).await;
            }
        })
        .detach();
    }

    fn check_feed(&mut self, updater: Arc<NativeUpdater>, manual: bool, cx: &mut Context<Self>) {
        self.update = Update::Checking;
        let reply = updater.check(&self.runtime, CancellationToken::new());
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                if !matches!(this.update, Update::Checking) {
                    return;
                }
                this.update = match result {
                    Ok(Some(release)) => Update::Available(updater, release),
                    Ok(None) => {
                        if manual {
                            toasts::info("This native build is up to date.", cx);
                        }
                        Update::None
                    }
                    Err(error) => {
                        if manual {
                            toasts::error(error.to_string(), cx);
                        }
                        Update::None
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn download_update(&mut self, cx: &mut Context<Self>) {
        let (updater, release) = match std::mem::take(&mut self.update) {
            Update::Available(updater, release) | Update::Failed(updater, release, _) => {
                (updater, release)
            }
            other => {
                self.update = other;
                return;
            }
        };
        let progress = Arc::new(Progress::default());
        let reply = {
            let progress = progress.clone();
            updater.download_with_progress(
                &self.runtime,
                release.clone(),
                CancellationToken::new(),
                move |chunk, total| {
                    progress
                        .downloaded
                        .fetch_add(chunk as u64, Ordering::Relaxed);
                    progress.total.store(total.unwrap_or(0), Ordering::Relaxed);
                },
            )
        };
        self.update = Update::Downloading(version(&release), progress);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let download = async { reply?.receive().await };
            futures::pin_mut!(download);
            // The percentage lives in atomics; redraw while it moves.
            let result = loop {
                let tick = cx.background_executor().timer(Duration::from_millis(250));
                futures::pin_mut!(tick);
                match futures::future::select(download.as_mut(), tick).await {
                    futures::future::Either::Left((result, _)) => break result,
                    futures::future::Either::Right(_) => {
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            return;
                        }
                    }
                }
            };
            let _ = this.update(cx, |this, cx| {
                this.update = match result {
                    Ok(update) => Update::Ready(updater, Some(update)),
                    Err(error) => Update::Failed(updater, release, error.to_string().into()),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// Shipping's Restart: installs the verified update and relaunches.
    pub(super) fn install_update(&mut self, cx: &mut Context<Self>) {
        if !self.update.ready() {
            return;
        }
        if self.meeting_live() {
            toasts::error("Finish recording before installing the update.", cx);
            return;
        }
        let Some(installation) =
            std::env::var_os("ANARLOG_NATIVE_UPDATE_INSTALLATION").map(PathBuf::from)
        else {
            toasts::error(
                "Set ANARLOG_NATIVE_UPDATE_INSTALLATION to the GPUI package to install its signed update.",
                cx,
            );
            return;
        };
        if self
            .product
            .as_ref()
            .is_some_and(|view| !view.update(cx, |view, cx| view.can_close(cx)))
        {
            return;
        }
        let Update::Ready(updater, Some(update)) = std::mem::take(&mut self.update) else {
            return;
        };
        let release = update.release.clone();
        self.update = Update::Ready(updater.clone(), None);
        let runtime = self.runtime.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result: desktop_runtime::Result<()> = async {
                let pause = this
                    .update(cx, |this, cx| this.pause_writers(cx))
                    .map_err(|_| ServiceError::Closed)?;
                pause.await?;
                let installer = crate::platform::updater::NativeInstaller {
                    kind: if cfg!(target_os = "macos") {
                        crate::platform::updater::PackageKind::MacBundle
                    } else if cfg!(target_os = "windows") {
                        crate::platform::updater::PackageKind::WindowsNsis
                    } else {
                        crate::platform::updater::PackageKind::AppImage
                    },
                    staging_root: installation
                        .parent()
                        .ok_or_else(|| ServiceError::Failed("Invalid installation path".into()))?
                        .to_owned(),
                    installation,
                };
                runtime
                    .service(move |_| async move { installer.install(update).await })?
                    .receive()
                    .await
            }
            .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    crate::platform::windows::request_restart();
                    this.request_quit(cx);
                }
                Err(error) => {
                    this.resume_writers(cx);
                    this.update = Update::Failed(updater, release, error.to_string().into());
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
