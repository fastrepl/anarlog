use crate::models::PermissionStatus;

#[cfg(target_os = "macos")]
use block2::StackBlock;
#[cfg(target_os = "macos")]
use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeAudio};
#[cfg(target_os = "macos")]
use objc2_contacts::{CNContactStore, CNEntityType};
#[cfg(target_os = "macos")]
use objc2_event_kit::{EKEntityType, EKEventStore};

#[allow(unused_macros)]
macro_rules! check {
    ($permission:literal, $raw:expr) => {{
        let raw = $raw;
        let status: PermissionStatus = raw.into();
        tracing::debug!(permission = $permission, ?raw, ?status);
        Ok(status)
    }};
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    Calendar,
    Contacts,
    Microphone,
    SystemAudio,
    Accessibility,
}

pub struct Permissions<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(dead_code)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Permissions<'a, R, M> {
    pub async fn open(&self, permission: Permission) -> Result<(), crate::Error> {
        match permission {
            Permission::Calendar => self.open_calendar().await,
            Permission::Contacts => self.open_contacts().await,
            Permission::Microphone => self.open_microphone().await,
            Permission::SystemAudio => self.open_system_audio().await,
            Permission::Accessibility => self.open_accessibility().await,
        }
    }

    pub async fn check(&self, permission: Permission) -> Result<PermissionStatus, crate::Error> {
        match permission {
            Permission::Calendar => self.check_calendar().await,
            Permission::Contacts => self.check_contacts().await,
            Permission::Microphone => self.check_microphone().await,
            Permission::SystemAudio => self.check_system_audio().await,
            Permission::Accessibility => self.check_accessibility().await,
        }
    }

    pub async fn request(&self, permission: Permission) -> Result<(), crate::Error> {
        match permission {
            Permission::Calendar => self.request_calendar().await,
            Permission::Contacts => self.request_contacts().await,
            Permission::Microphone => self.request_microphone().await,
            Permission::SystemAudio => self.request_system_audio().await,
            Permission::Accessibility => self.request_accessibility().await,
        }
    }

    pub async fn reset(&self, permission: Permission) -> Result<(), crate::Error> {
        match permission {
            Permission::Calendar => self.reset_calendar().await,
            Permission::Contacts => self.reset_contacts().await,
            Permission::Microphone => self.reset_microphone().await,
            Permission::SystemAudio => self.reset_system_audio().await,
            Permission::Accessibility => self.reset_accessibility().await,
        }
    }

    async fn open_calendar(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars")
                .spawn()?
                .wait()?;
        }

        Ok(())
    }

    async fn open_contacts(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Contacts")
                .spawn()?
                .wait()?;
        }

        Ok(())
    }

    async fn open_microphone(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
                .spawn()?
                .wait()?;
        }

        Ok(())
    }

    async fn open_system_audio(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
                )
                .spawn()?
                .wait()?;
        }

        Ok(())
    }

    async fn open_accessibility(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                )
                .spawn()?
                .wait()?;
        }

        Ok(())
    }

    async fn check_calendar(&self) -> Result<PermissionStatus, crate::Error> {
        #[cfg(target_os = "macos")]
        return check!("calendar", unsafe {
            EKEventStore::authorizationStatusForEntityType(EKEntityType::Event)
        });

        #[cfg(not(target_os = "macos"))]
        {
            Ok(PermissionStatus::Denied)
        }
    }

    async fn check_contacts(&self) -> Result<PermissionStatus, crate::Error> {
        #[cfg(target_os = "macos")]
        return check!("contacts", unsafe {
            CNContactStore::authorizationStatusForEntityType(CNEntityType::Contacts)
        });

        #[cfg(not(target_os = "macos"))]
        {
            Ok(PermissionStatus::Denied)
        }
    }

    async fn check_microphone(&self) -> Result<PermissionStatus, crate::Error> {
        #[cfg(target_os = "macos")]
        return check!("microphone", unsafe {
            let media_type = AVMediaTypeAudio.unwrap();
            AVCaptureDevice::authorizationStatusForMediaType(media_type)
        });

        #[cfg(not(target_os = "macos"))]
        {
            use futures_util::StreamExt;
            let mut mic_sample_stream = hypr_audio::AudioInput::from_mic(None)?.stream();
            let sample = mic_sample_stream.next().await;
            Ok(if sample.is_some() {
                PermissionStatus::Authorized
            } else {
                PermissionStatus::Denied
            })
        }
    }

    async fn check_system_audio(&self) -> Result<PermissionStatus, crate::Error> {
        #[cfg(target_os = "macos")]
        return check!("system_audio", hypr_tcc::audio_capture_permission_status());

        #[cfg(not(target_os = "macos"))]
        {
            use futures_util::StreamExt;
            let mut speaker_sample_stream = hypr_audio::AudioInput::from_speaker().stream();
            let sample = speaker_sample_stream.next().await;
            Ok(if sample.is_some() {
                PermissionStatus::Authorized
            } else {
                PermissionStatus::Denied
            })
        }
    }

    async fn check_accessibility(&self) -> Result<PermissionStatus, crate::Error> {
        #[cfg(target_os = "macos")]
        return check!(
            "accessibility",
            macos_accessibility_client::accessibility::application_is_trusted()
        );

        #[cfg(not(target_os = "macos"))]
        {
            Ok(PermissionStatus::Denied)
        }
    }

    async fn request_calendar(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            use objc2_foundation::NSError;

            let event_store = unsafe { EKEventStore::new() };
            let (tx, rx) = std::sync::mpsc::channel::<bool>();
            let completion =
                block2::RcBlock::new(move |granted: objc2::runtime::Bool, _error: *mut NSError| {
                    let _ = tx.send(granted.as_bool());
                });

            unsafe {
                event_store
                    .requestFullAccessToEventsWithCompletion(&*completion as *const _ as *mut _)
            };

            let _ = rx.recv_timeout(std::time::Duration::from_secs(60));
        }

        Ok(())
    }

    async fn request_contacts(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            use objc2_foundation::NSError;

            let contacts_store = unsafe { CNContactStore::new() };
            let (tx, rx) = std::sync::mpsc::channel::<bool>();
            let completion =
                block2::RcBlock::new(move |granted: objc2::runtime::Bool, _error: *mut NSError| {
                    let _ = tx.send(granted.as_bool());
                });

            unsafe {
                contacts_store.requestAccessForEntityType_completionHandler(
                    CNEntityType::Contacts,
                    &completion,
                );
            };

            let _ = rx.recv_timeout(std::time::Duration::from_secs(60));
        }

        Ok(())
    }

    async fn request_microphone(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            unsafe {
                let media_type = AVMediaTypeAudio.unwrap();
                let block = StackBlock::new(|_granted| {});
                AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &block);
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            use futures_util::StreamExt;
            let mut mic_sample_stream = hypr_audio::AudioInput::from_mic(None)?.stream();
            mic_sample_stream.next().await;
        }

        Ok(())
    }

    async fn request_system_audio(&self) -> Result<(), crate::Error> {
        let stop = hypr_audio::AudioOutput::silence();

        use futures_util::StreamExt;
        let mut speaker_sample_stream = hypr_audio::AudioInput::from_speaker().stream();
        speaker_sample_stream.next().await;

        let _ = stop.send(());
        Ok(())
    }

    async fn request_accessibility(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            macos_accessibility_client::accessibility::application_is_trusted_with_prompt();
        }

        Ok(())
    }

    async fn reset_calendar(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        self.reset_tcc("Calendar").await;

        Ok(())
    }

    async fn reset_contacts(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        self.reset_tcc("AddressBook").await;

        Ok(())
    }

    async fn reset_microphone(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        self.reset_tcc("Microphone").await;

        Ok(())
    }

    async fn reset_system_audio(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        self.reset_tcc("AudioCapture").await;

        Ok(())
    }

    async fn reset_accessibility(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        self.reset_tcc("Accessibility").await;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn reset_tcc(&self, service: &str) {
        use tauri_plugin_shell::ShellExt;

        let bundle_id = if cfg!(debug_assertions) {
            match hypr_bundle::get_ancestor_bundle_id() {
                Some(id) => {
                    tracing::info!(service, bundle_id = %id, "resolving_ancestor_bundle_id");
                    id
                }
                None => {
                    tracing::warn!(service, "skipping_tcc_reset");
                    return;
                }
            }
        } else {
            self.manager.config().identifier.clone()
        };

        let _ = self
            .manager
            .shell()
            .command("tccutil")
            .args(["reset", service, &bundle_id])
            .output()
            .await;
    }
}

pub trait PermissionsPluginExt<R: tauri::Runtime> {
    fn permissions(&self) -> Permissions<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> PermissionsPluginExt<R> for T {
    fn permissions(&self) -> Permissions<'_, R, Self>
    where
        Self: Sized,
    {
        Permissions {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
