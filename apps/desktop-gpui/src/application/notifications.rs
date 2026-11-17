//! Shipping's sidebar toast registry (`sidebar/toast/{registry,index}.tsx`): at most one
//! app notice at a time, the first whose condition holds and that the user has not dismissed.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gpui::{Context, SharedString, Window};
use serde_json::{Value, json};

use super::{ApplicationView, toasts};
use crate::{
    contracts::ProductRoute,
    platform::tray::UpdateMenu,
    product::settings::{self, Snapshot},
};

const DISMISSALS_KEY: &str = "gpui_toast_dismissals";
const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Default)]
pub(super) struct Notifications {
    preferences: Option<Arc<Snapshot>>,
    /// The notice on screen: its toast id and presentation.
    shown: Option<(String, String)>,
    session_dismissed: HashSet<String>,
    /// Permanent dismissals and day snoozes; `None` until read.
    stored: Option<Stored>,
    update_failed: bool,
    downloading: bool,
    downloads: Vec<(String, u8)>,
}

#[derive(Clone, Default, Debug, PartialEq)]
struct Stored {
    dismissed: Vec<String>,
    snoozed: HashMap<String, i64>,
}

impl Stored {
    fn decode(value: Option<&Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };
        Self {
            dismissed: value["dismissed"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|id| id.as_str().map(str::to_owned))
                .collect(),
            snoozed: value["snoozed"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(id, until)| Some((id.clone(), until.as_i64()?)))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lifecycle {
    /// Shown while its condition holds; no Dismiss button.
    Condition,
    Session,
    Day,
    Permanent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    SignIn,
    Transcription,
    Intelligence,
    Download,
    Install,
}

#[derive(Clone, Debug, PartialEq)]
struct Notice {
    id: String,
    dismissal: String,
    message: String,
    detail: Option<String>,
    action: Option<(&'static str, Act)>,
    lifecycle: Lifecycle,
    error: bool,
    loading: bool,
    logo: bool,
}

impl Notice {
    fn new(id: impl Into<String>, message: impl Into<String>, lifecycle: Lifecycle) -> Self {
        let id = id.into();
        Self {
            dismissal: id.clone(),
            id,
            message: message.into(),
            detail: None,
            action: None,
            lifecycle,
            error: false,
            loading: false,
            logo: false,
        }
    }

    fn presentation(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.id,
            self.message,
            self.detail.as_deref().unwrap_or_default(),
            self.error,
            self.loading,
            self.action.map(|(label, _)| label).unwrap_or_default()
        )
    }
}

struct Inputs<'a> {
    /// `None` while auth is resolving, as it stays when cloud sign-in is not configured.
    signed_in: Option<bool>,
    stt: (&'a str, &'a str),
    llm: (&'a str, &'a str),
    section: Option<&'a str>,
    live: bool,
    downloads: &'a [(String, u8)],
    update: &'a UpdateMenu,
    /// Restart was chosen; shipping hides the update actions while it installs.
    installing: bool,
}

fn configured_stt(provider: &str, model: &str) -> bool {
    if provider.is_empty() || model.is_empty() {
        return false;
    }
    match provider {
        "anarlog" => {
            model == "cloud"
                || model.starts_with("soniqo-")
                || model == "apple-speech"
                || model.starts_with("am-")
                || model.starts_with("Quantized")
        }
        "soniqo" => model.starts_with("soniqo-"),
        "apple_speech" => model == "apple-speech",
        "local_file" => model == "local-file",
        _ => true,
    }
}

/// Shipping's `createDesktopUpdateToast` over the lifecycle's update state.
fn update_notice(update: &UpdateMenu, installing: bool) -> Option<Notice> {
    Some(match update {
        UpdateMenu::CheckForUpdate => return None,
        UpdateMenu::RestartToApply(version) => Notice {
            action: (!installing).then_some(("Restart", Act::Install)),
            ..Notice::new(
                format!("desktop-update:{version}:ready"),
                format!("Anarlog {version} is ready to install"),
                Lifecycle::Session,
            )
        },
        UpdateMenu::Downloading(version, percent) => Notice {
            loading: true,
            ..Notice::new(
                format!("desktop-update:{version}:downloading"),
                format!(
                    "Downloading Anarlog {version}{}",
                    percent
                        .map(|percent| format!(" ({percent}%)"))
                        .unwrap_or_default()
                ),
                Lifecycle::Session,
            )
        },
        UpdateMenu::Failed(version, message) => Notice {
            action: (!installing).then_some(("Retry", Act::Download)),
            error: true,
            ..Notice::new(
                format!("desktop-update:{version}:failed"),
                if message.is_empty() {
                    "The update download failed".to_owned()
                } else {
                    message.to_string()
                },
                Lifecycle::Session,
            )
        },
        UpdateMenu::Available(version) => Notice {
            action: Some(("Download", Act::Download)),
            ..Notice::new(
                format!("desktop-update:{version}:available"),
                format!("Anarlog {version} is available"),
                Lifecycle::Day,
            )
        },
    })
}

/// `createToastRegistry`'s entries whose condition holds, in priority order.
fn registry(inputs: &Inputs) -> Vec<Notice> {
    let loading = inputs.signed_in.is_none();
    let authed = inputs.signed_in == Some(true);
    let has_stt = configured_stt(inputs.stt.0, inputs.stt.1);
    let has_llm = !inputs.llm.0.is_empty() && !inputs.llm.1.is_empty();
    let pro_stt = inputs.stt == ("anarlog", "cloud");
    let pro_llm = inputs.llm.0 == "anarlog";
    let usable_stt = has_stt && (loading || authed || !pro_stt);
    let usable_llm = has_llm && (loading || authed || !pro_llm);
    let signed_out = inputs.signed_in == Some(false);
    let mut notices = Vec::new();
    if !inputs.downloads.is_empty() {
        let single = inputs.downloads.len() == 1;
        notices.push(Notice {
            detail: Some(
                inputs
                    .downloads
                    .iter()
                    .map(|(name, percent)| {
                        if single {
                            format!("{percent}%")
                        } else {
                            format!("{name}: {percent}%")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            loading: true,
            ..Notice::new(
                "downloading-model",
                if single {
                    format!("Downloading {}", inputs.downloads[0].0)
                } else {
                    format!("Downloading {} models", inputs.downloads.len())
                },
                Lifecycle::Session,
            )
        });
    }
    // Never show update prompts mid-meeting; they resurface after.
    if let Some(update) = update_notice(inputs.update, inputs.installing).filter(|_| !inputs.live) {
        notices.push(update);
    }
    if signed_out {
        notices.push(Notice {
            dismissal: "auth-promotion".into(),
            action: Some(("Sign in", Act::SignIn)),
            logo: true,
            ..Notice::new(
                "sign-in-benefits",
                "Sign in to get the most out of Anarlog",
                Lifecycle::Permanent,
            )
        });
    }
    if !usable_stt && inputs.section != Some("transcription") {
        notices.push(Notice {
            action: Some(("Add", Act::Transcription)),
            ..Notice::new(
                "missing-stt",
                "Transcription provider needed",
                Lifecycle::Condition,
            )
        });
    }
    if usable_stt && !usable_llm && inputs.section != Some("intelligence") {
        notices.push(Notice {
            action: Some(("Add", Act::Intelligence)),
            ..Notice::new("missing-llm", "Language model needed", Lifecycle::Condition)
        });
    }
    if signed_out && has_llm && has_stt && !pro_stt && !pro_llm {
        notices.push(Notice {
            dismissal: "auth-promotion".into(),
            action: Some(("Upgrade", Act::SignIn)),
            ..Notice::new(
                "upgrade-to-pro",
                "Pro features available",
                Lifecycle::Permanent,
            )
        });
    }
    notices
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

impl Notifications {
    fn dismissed(&self, notice: &Notice, now: i64) -> bool {
        let Some(stored) = &self.stored else {
            // Persisted dismissals are still loading.
            return notice.lifecycle != Lifecycle::Condition;
        };
        match notice.lifecycle {
            Lifecycle::Condition => false,
            Lifecycle::Permanent => stored.dismissed.contains(&notice.dismissal),
            Lifecycle::Day if stored.snoozed.get(&notice.dismissal) > Some(&now) => true,
            _ => self.session_dismissed.contains(&notice.dismissal),
        }
    }

    fn current(&self, notices: Vec<Notice>, now: i64) -> Option<Notice> {
        notices
            .into_iter()
            .find(|notice| !self.dismissed(notice, now))
    }

    /// Shipping's `floating_bar_enabled` setting, on by default.
    pub(super) fn floating_bar_enabled(&self) -> bool {
        self.preferences
            .as_ref()
            .and_then(|snapshot| snapshot.get("floating_bar_enabled"))
            .and_then(|setting| setting.value.as_bool())
            .unwrap_or(true)
    }
}

fn choice<'a>(snapshot: &'a Snapshot, key: &str) -> &'a str {
    snapshot
        .get(key)
        .and_then(|setting| setting.value.as_str())
        .unwrap_or_default()
}

impl ApplicationView {
    pub(super) fn watch_preferences(&mut self, cx: &mut Context<Self>) {
        let reply = settings::watch(&self.runtime);
        let runtime = self.runtime.clone();
        self.preferences_task = Some(cx.spawn(async move |this, cx| {
            let stored =
                crate::product::local::data::setting_value(&runtime, DISMISSALS_KEY.into()).await;
            let _ = this.update(cx, |this, _| {
                this.notifications.stored = Some(Stored::decode(stored.ok().flatten().as_ref()))
            });
            let result = async { reply?.receive().await }.await;
            let mut watch = match result {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                    return;
                }
            };
            // Like shipping, notices wait half a second after launch.
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            loop {
                if let Some(error) = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok())
                {
                    let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                    break;
                }
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let snapshot = cx
                    .background_executor()
                    .spawn(async move { settings::decode(&rows).map(Arc::new) })
                    .await;
                if this
                    .update(cx, |this, cx| {
                        match snapshot {
                            Ok(snapshot) => {
                                let use_24_hour_time = snapshot
                                    .get("use_24_hour_time")
                                    .and_then(|setting| setting.value.as_bool())
                                    .unwrap_or(false);
                                let timezone = choice(&snapshot, "timezone").parse().ok();
                                let show_folder = snapshot
                                    .get("sidebar_show_folder")
                                    .and_then(|setting| setting.value.as_bool())
                                    .unwrap_or(true);
                                let show_tags = snapshot
                                    .get("sidebar_show_tags")
                                    .and_then(|setting| setting.value.as_bool())
                                    .unwrap_or(false);
                                let week_start = crate::workspace::calendar::week_start(
                                    choice(&snapshot, "week_start"),
                                    sys_locale::get_locale().as_deref(),
                                );
                                this.workspace.update(cx, |workspace, cx| {
                                    workspace.set_timeline_clock(use_24_hour_time, timezone, cx);
                                    workspace.set_timeline_metadata(show_folder, show_tags, cx);
                                    workspace.set_calendar_week_start(week_start, cx);
                                    workspace.set_floating_bar_enabled(
                                        snapshot
                                            .get("floating_bar_enabled")
                                            .and_then(|setting| setting.value.as_bool())
                                            .unwrap_or(true),
                                        cx,
                                    );
                                });
                                this.automation.update(cx, |automation, _| {
                                    automation.set_preferences(snapshot.clone())
                                });
                                this.schedule_preferences(snapshot.clone(), cx);
                                this.notifications.preferences = Some(snapshot);
                            }
                            Err(error) => {
                                this.notifications.preferences = None;
                                this.status(&error.to_string(), cx);
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                if watch.snapshots.changed().await.is_err() {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        }));
    }

    /// Model download percentages have no event; the host tick redraws when they move.
    pub(super) fn poll_downloads(&mut self, cx: &mut Context<Self>) {
        let downloads = self
            .native
            .as_ref()
            .map(|native| native.local.models.active_downloads())
            .unwrap_or_default();
        if downloads != self.notifications.downloads {
            self.notifications.downloads = downloads;
            cx.notify();
        }
    }

    /// The registry's notice for the current state; `None` before preferences load and while
    /// onboarding, quitting or updating.
    fn notice(&mut self, cx: &mut Context<Self>) -> Option<Notice> {
        if self.closing || self.writers_paused {
            return None;
        }
        let snapshot = self.notifications.preferences.clone()?;
        let product = self.product.as_ref().map(|product| product.read(cx));
        if product.is_some_and(|product| matches!(product.route, ProductRoute::Onboarding)) {
            return None;
        }
        let section = product.map(|product| product.active_section());
        let cloud = self
            .native
            .as_ref()
            .is_some_and(|native| native.cloud.service.is_some());
        let signed_in = cx
            .try_global::<crate::product::account::Account>()
            .filter(|_| cloud)
            .map(|account| account.signed_in());
        let downloads = self.notifications.downloads.clone();
        let update = self.update_menu();
        // Session dismissals of a failed update or a finished download do not outlive them.
        let failed = matches!(update, UpdateMenu::Failed(..));
        if self.notifications.update_failed && !failed {
            self.notifications
                .session_dismissed
                .retain(|id| !id.ends_with(":failed"));
        }
        self.notifications.update_failed = failed;
        if self.notifications.downloading && downloads.is_empty() {
            self.notifications
                .session_dismissed
                .remove("downloading-model");
        }
        self.notifications.downloading = !downloads.is_empty();
        let notices = registry(&Inputs {
            signed_in,
            stt: (
                choice(&snapshot, "current_stt_provider"),
                choice(&snapshot, "current_stt_model"),
            ),
            llm: (
                choice(&snapshot, "current_llm_provider"),
                choice(&snapshot, "current_llm_model"),
            ),
            section,
            live: self.meeting_live(),
            downloads: &downloads,
            update: &update,
            installing: self.update_installing(),
        });
        self.notifications.current(notices, now_ms())
    }

    /// Shows, replaces or removes the registry's toast after a render that changed it.
    pub(super) fn sync_notice(&mut self, window: &Window, cx: &mut Context<Self>) {
        let notice = self.notice(cx);
        let presentation = notice.as_ref().map(Notice::presentation);
        if self.notifications.shown.as_ref().map(|(_, shown)| shown) == presentation.as_ref() {
            return;
        }
        let previous = self.notifications.shown.take();
        self.notifications.shown = notice
            .as_ref()
            .zip(presentation)
            .map(|(notice, presentation)| (notice.id.clone(), presentation));
        cx.defer_in(window, move |this, _, cx| {
            if let Some((id, _)) = previous
                && notice.as_ref().is_none_or(|notice| notice.id != id)
            {
                toasts::dismiss(&id, cx);
            }
            if let Some(notice) = notice {
                toasts::show(this.notice_toast(notice, cx), cx);
            }
        });
    }

    fn notice_toast(&self, notice: Notice, cx: &mut Context<Self>) -> toasts::Toast {
        let view = cx.entity().downgrade();
        let mut toast = if notice.loading {
            toasts::Toast::loading(notice.message.clone())
        } else if notice.error {
            toasts::Toast::error(notice.message.clone())
        } else {
            toasts::Toast::info(notice.message.clone()).sticky()
        }
        .id(SharedString::from(notice.id.clone()))
        .dismissible(notice.lifecycle != Lifecycle::Condition);
        if let Some(detail) = notice.detail.clone() {
            toast = toast.description(detail);
        }
        if notice.logo {
            toast = toast.image("product/AnarlogIcon.png");
        }
        if let Some((label, act)) = notice.action {
            let view = view.clone();
            toast = toast.action(label, move |window, cx| {
                let _ = view.update(cx, |this, cx| this.notice_action(act, window, cx));
            });
        }
        toast.on_dismiss(move |_, cx| {
            let _ = view.update(cx, |this, cx| this.dismiss_notice(&notice, cx));
        })
    }

    fn notice_action(&mut self, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        match act {
            Act::SignIn => {
                if let Some(cloud) = self
                    .native
                    .as_ref()
                    .and_then(|native| native.cloud.service.clone())
                {
                    cx.spawn(async move |this, cx| {
                        if let Err(error) = cloud.sign_in().await {
                            let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                        }
                    })
                    .detach();
                }
            }
            Act::Transcription => self.open_product(ProductRoute::Transcription, window, cx),
            Act::Intelligence => self.open_product(ProductRoute::Intelligence, window, cx),
            Act::Download => self.download_update(false, cx),
            Act::Install => self.apply_update(false, cx),
        }
    }

    fn dismiss_notice(&mut self, notice: &Notice, cx: &mut Context<Self>) {
        let stored = self.notifications.stored.get_or_insert_default();
        match notice.lifecycle {
            Lifecycle::Condition => return,
            Lifecycle::Session => {
                self.notifications
                    .session_dismissed
                    .insert(notice.dismissal.clone());
                return;
            }
            Lifecycle::Permanent if !stored.dismissed.contains(&notice.dismissal) => {
                stored.dismissed.push(notice.dismissal.clone())
            }
            Lifecycle::Permanent => return,
            Lifecycle::Day => {
                stored
                    .snoozed
                    .insert(notice.dismissal.clone(), now_ms() + DAY_MS);
            }
        }
        let value = json!({"dismissed": stored.dismissed, "snoozed": stored.snoozed}).to_string();
        let reply = self.runtime.submit(move |services| async move {
            services
                .executor
                .execute(
                    "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at".into(),
                    vec![json!(DISMISSALS_KEY), json!(value)],
                )
                .await
                .map(|_| ())
                .map_err(|error| desktop_runtime::ServiceError::Failed(error.to_string().into()))
        });
        cx.spawn(async move |this, cx| {
            if let Err(error) = async { reply?.receive().await }.await {
                let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs<'a>(stt: (&'a str, &'a str), llm: (&'a str, &'a str)) -> Inputs<'a> {
        Inputs {
            signed_in: None,
            stt,
            llm,
            section: None,
            live: false,
            downloads: &[],
            update: &UpdateMenu::CheckForUpdate,
            installing: false,
        }
    }

    fn ids(notices: &[Notice]) -> Vec<&str> {
        notices.iter().map(|notice| notice.id.as_str()).collect()
    }

    #[test]
    fn registry_orders_notices_and_honors_auth_tabs_meetings_and_dismissals() {
        let missing = ("", "");
        let local = ("anarlog", "soniqo-parakeet-streaming");
        let cloud = ("anarlog", "cloud");
        let byok = ("openai", "gpt-4o");
        assert_eq!(ids(&registry(&inputs(missing, missing))), ["missing-stt"]);
        assert!(
            registry(&Inputs {
                section: Some("transcription"),
                ..inputs(missing, missing)
            })
            .is_empty()
        );
        assert_eq!(ids(&registry(&inputs(local, missing))), ["missing-llm"]);
        // While auth resolves, hosted providers count as usable.
        assert!(registry(&inputs(cloud, ("anarlog", "auto"))).is_empty());
        let signed_out = Inputs {
            signed_in: Some(false),
            ..inputs(cloud, byok)
        };
        assert_eq!(
            ids(&registry(&signed_out)),
            ["sign-in-benefits", "missing-stt"]
        );
        let update = UpdateMenu::Available("1.2.0".into());
        let downloads = [("Parakeet".to_owned(), 40)];
        let busy = Inputs {
            signed_in: Some(false),
            downloads: &downloads,
            update: &update,
            ..inputs(local, byok)
        };
        assert_eq!(
            ids(&registry(&busy)),
            [
                "downloading-model",
                "desktop-update:1.2.0:available",
                "sign-in-benefits",
                "upgrade-to-pro"
            ]
        );
        assert_eq!(registry(&busy)[0].message, "Downloading Parakeet");
        let live = Inputs {
            live: true,
            downloads: &[],
            ..busy
        };
        assert_eq!(
            ids(&registry(&live)),
            ["sign-in-benefits", "upgrade-to-pro"]
        );

        let mut notifications = Notifications::default();
        let now = now_ms();
        let notices = registry(&Inputs {
            update: &update,
            ..signed_out
        });
        assert_eq!(
            notifications
                .current(notices.clone(), now)
                .map(|notice| notice.id),
            Some("missing-stt".into()),
            "persistent notices wait for stored dismissals"
        );
        notifications.stored = Some(Stored {
            dismissed: vec!["auth-promotion".into()],
            snoozed: HashMap::from([("desktop-update:1.2.0:available".into(), now + DAY_MS)]),
        });
        assert_eq!(
            notifications
                .current(notices.clone(), now)
                .map(|notice| notice.id),
            Some("missing-stt".into())
        );
        notifications.stored = Some(Stored::default());
        assert_eq!(
            notifications.current(notices, now).map(|notice| notice.id),
            Some("desktop-update:1.2.0:available".into())
        );
        assert_eq!(
            Stored::decode(Some(
                &json!({"dismissed": ["auth-promotion"], "snoozed": {"a": 5}})
            )),
            Stored {
                dismissed: vec!["auth-promotion".into()],
                snoozed: HashMap::from([("a".into(), 5)]),
            }
        );
    }

    #[test]
    fn update_toast_follows_the_lifecycle_update_state() {
        let notice = |update: UpdateMenu, installing| update_notice(&update, installing);
        assert_eq!(notice(UpdateMenu::CheckForUpdate, false), None);
        let downloading = notice(UpdateMenu::Downloading("1.2.0".into(), Some(58)), false).unwrap();
        assert_eq!(downloading.message, "Downloading Anarlog 1.2.0 (58%)");
        assert!(downloading.loading && downloading.action.is_none());
        let ready = notice(UpdateMenu::RestartToApply("1.2.0".into()), false).unwrap();
        assert_eq!(ready.action, Some(("Restart", Act::Install)));
        let installing = notice(UpdateMenu::RestartToApply("1.2.0".into()), true).unwrap();
        assert_eq!(installing.action, None);
        let failed = notice(UpdateMenu::Failed("1.2.0".into(), "".into()), false).unwrap();
        assert_eq!(
            (failed.message.as_str(), failed.action, failed.error),
            (
                "The update download failed",
                Some(("Retry", Act::Download)),
                true
            )
        );
    }
}
