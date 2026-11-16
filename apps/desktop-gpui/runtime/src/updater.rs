use std::sync::Arc;

use updater_core::UpdateChecker;
pub use updater_core::UpdateConfig;

use crate::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError, types::failure};

pub use updater_core::ResolvedRelease;

pub struct NativeUpdater {
    checker: Arc<UpdateChecker>,
}

pub struct VerifiedUpdate {
    pub release: ResolvedRelease,
    bytes: Vec<u8>,
}

impl VerifiedUpdate {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InstallConditions {
    pub meeting_active: bool,
    pub onboarding: bool,
    pub automatic: bool,
}

pub trait UpdateInstaller {
    fn install(&self, update: VerifiedUpdate) -> futures::future::BoxFuture<'static, Result<()>>;
}

impl NativeUpdater {
    /// The distribution owner must supply GPUI-specific signed endpoints and target.
    pub fn new(config: UpdateConfig) -> Result<Self> {
        if config.pubkey.is_empty() || config.endpoints.iter().any(|url| url.scheme() != "https") {
            return Err(ServiceError::Unsupported(
                "Native updates require HTTPS endpoints and a signing public key".into(),
            ));
        }
        Ok(Self {
            checker: Arc::new(UpdateChecker::new(config).map_err(failure)?),
        })
    }

    pub fn check(
        &self,
        runtime: &RuntimeHandle,
        cancel: CancellationToken,
    ) -> Result<Reply<Option<ResolvedRelease>>> {
        let checker = self.checker.clone();
        runtime.service(move |services| async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(ServiceError::Cancelled),
                _ = services.shutdown_requested.cancelled() => Err(ServiceError::Cancelled),
                result = checker.check() => result.map_err(failure),
            }
        })
    }

    pub fn download(
        &self,
        runtime: &RuntimeHandle,
        release: ResolvedRelease,
        cancel: CancellationToken,
    ) -> Result<Reply<VerifiedUpdate>> {
        let checker = self.checker.clone();
        runtime.service(move |services| async move {
            if release.download_url.scheme() != "https" {
                return Err(ServiceError::Unsupported(
                    "Native update download requires HTTPS".into(),
                ));
            }
            let bytes = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(ServiceError::Cancelled),
                _ = services.shutdown_requested.cancelled() => return Err(ServiceError::Cancelled),
                result = checker.download(&release, |_, _| {}) => result.map_err(failure)?,
            };
            Ok(VerifiedUpdate { release, bytes })
        })
    }

    pub fn permit_install(conditions: InstallConditions) -> Result<()> {
        if conditions.meeting_active {
            return Err(ServiceError::Unsupported(
                "Update deferred until the meeting ends".into(),
            ));
        }
        if conditions.automatic && conditions.onboarding {
            return Err(ServiceError::Unsupported(
                "Automatic relaunch deferred until onboarding completes".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_defer_during_meetings_and_automatic_onboarding_relaunch() {
        assert!(
            NativeUpdater::permit_install(InstallConditions {
                meeting_active: true,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            NativeUpdater::permit_install(InstallConditions {
                automatic: true,
                onboarding: true,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            NativeUpdater::permit_install(InstallConditions {
                onboarding: true,
                ..Default::default()
            })
            .is_ok()
        );
        assert!(updater_core::verify_signature(b"untrusted", "invalid", "invalid").is_err());
    }
}
