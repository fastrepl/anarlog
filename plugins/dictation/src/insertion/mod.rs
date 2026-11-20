//! The field a dictation goes into: capture, privacy-gated reads, guarded writes and output
//! ducking.
//!
//! A captured field belongs to the one dictation that first delivers into it. It takes that
//! delivery only while the dictation can still deliver, then, if the delivery applied, its raw
//! restoration and one learning read for `AFTER_DELIVERY`. Releasing the field, or retiring its
//! account or every dictation (a meeting), ends that at once, also for a write already queued.

mod edit;
mod volume;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

use std::{
    collections::{HashMap, VecDeque},
    sync::{LazyLock, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use dictation_core::{
    context::{
        ContextAccess, ContextExclusions, ContextSnapshot, DenyReason, WindowFacts, decide_context,
        may_read_for_fitting,
    },
    corrections::{FieldText, kept_text},
    terms::OrdinaryWords,
};
use serde::{Deserialize, Serialize};

use edit::{FieldPort, History};
pub use volume::output_ducker;

/// The most of the text before the caret that leaves this module for fitting.
const MAX_BEFORE_CHARS: usize = 1_000;
/// The longest selection a voice edit may send to the person's model.
const MAX_SELECTION_CHARS: usize = 20_000;
/// How much window text is read for a context snapshot, which keeps less.
const MAX_WINDOW_TEXT_CHARS: usize = 4_000;
const KEPT_TARGETS: usize = 8;
/// After an applied delivery, how long its field stays usable for putting back the raw words
/// (offered for 15 s) and the one learning read (made 20 s after), unless released sooner.
const AFTER_DELIVERY: Duration = Duration::from_secs(30);
/// Ended fields remembered, so a call still queued for one cannot claim it again.
const ENDED_TARGETS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextRequest {
    pub operation_id: String,
    /// The person's opt-in to window context for cleanup.
    #[serde(default)]
    pub external: bool,
    /// The person's opt-in to voice edits of a selection.
    #[serde(default)]
    pub selection: bool,
    /// `None` while the exclusions can't be read, which denies every read but local fitting.
    pub exclusions: Option<ContextExclusions>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TargetContext {
    /// Local only, for fitting: the end of the text before the caret.
    pub before: Option<String>,
    pub has_selection: bool,
    /// For a voice edit, when the field may be read and the person opted in.
    pub selection: Option<String>,
    /// Guards a voice edit: the field must still read the same when it is written.
    pub fingerprint: Option<String>,
    /// Window context for cleanup, when allowed and opted in.
    pub snapshot: Option<ContextSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InsertMode {
    /// The text as given, over the selection or at the caret.
    Caret,
    /// Fitted to the text before the caret now; with `canJoin`, a dictation that finishes the
    /// sentence goes over its period.
    Fit {
        #[serde(rename = "canJoin")]
        can_join: bool,
    },
    /// A voice edit of the selection the fingerprint was read with.
    ReplaceSelection { fingerprint: String },
    /// Puts the raw transcript back over what an applied edit typed, while that is still
    /// directly before the caret.
    Restore {
        #[serde(rename = "editId")]
        edit_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsertRequest {
    /// The dictation this write is for. A field takes only that dictation's writes.
    pub operation_id: String,
    /// Names one intended write; a write that may have landed is never made again.
    pub edit_id: String,
    pub text: String,
    pub mode: InsertMode,
    /// Fitting reads nothing in an excluded app.
    #[serde(default)]
    pub exclusions: Option<ContextExclusions>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum InsertMiss {
    FocusMoved,
    ContentChanged,
    Refused,
    Consumed,
    /// The dictation was cancelled, superseded or retired, or its field released, before
    /// anything was written.
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InsertOutcome {
    /// `verified`: read back from the field, not only reported by the app. `text`: what was typed.
    Applied { verified: bool, text: String },
    /// Provably nothing was written.
    Unchanged { reason: InsertMiss },
    /// The text may have landed. Never retry; offer it for copying.
    Uncertain,
}

pub async fn capture() -> Result<String, String> {
    platform::capture().await
}

pub async fn read_context(
    target: String,
    request: ContextRequest,
) -> Result<TargetContext, String> {
    blocking(move || Ok(context(&target, &request))).await
}

/// `deliverable`: the dictation's account while it can still deliver its final text; asked
/// before every step of the write.
pub async fn insert_dictation(
    target: String,
    request: InsertRequest,
    deliverable: impl Fn() -> Option<String> + Send + 'static,
) -> Result<InsertOutcome, String> {
    edit::validate(&request.text)?;
    blocking(move || {
        Ok(TARGETS.insert(&target, &request, &deliverable, || {
            let field = platform::field(&target).ok_or(InsertMiss::FocusMoved)?;
            let fitting = platform::facts(&target)
                .is_some_and(|facts| may_read_for_fitting(&facts, request.exclusions.as_ref()));
            Ok((field, fitting))
        }))
    })
    .await
}

/// What the person kept of an applied edit, from one fresh read of the same field.
pub async fn read_kept(
    target: String,
    request: ContextRequest,
    edit_id: String,
) -> Result<Option<String>, String> {
    blocking(move || {
        let Some(start) = TARGETS.kept_start(&target, &request.operation_id, &edit_id) else {
            return Ok(None);
        };
        if access(&target, &request) != ContextAccess::Allowed {
            return Ok(None);
        }
        let now = platform::field(&target).and_then(|field| field.read());
        Ok(now.and_then(|now| kept_text(&start, &now.text())))
    })
    .await
}

pub async fn release_target(target: String) -> Result<(), String> {
    // Before anything that waits on a write in progress, so a queued write never starts.
    TARGETS.end(&target);
    blocking(move || {
        TARGETS.forget(&target);
        platform::release(&target);
        Ok(())
    })
    .await
}

/// Ends the fields of `account`'s dictations, or of every dictation when `None` (a meeting).
pub fn retire_targets(account: Option<&str>) {
    let ended = TARGETS.retire(account);
    if ended.is_empty() {
        return;
    }
    tauri::async_runtime::spawn_blocking(move || {
        for target in &ended {
            TARGETS.forget(target);
            platform::release(target);
        }
    });
}

fn context(target: &str, request: &ContextRequest) -> TargetContext {
    if TARGETS.leases().is_ended(target) {
        return TargetContext::default();
    }
    let Some(field) = platform::field(target).filter(|field| field.focused()) else {
        return TargetContext::default();
    };
    // Every privacy fact is settled before any text of the field or window is read.
    let facts = platform::facts(target);
    let fitting = facts
        .as_ref()
        .is_some_and(|facts| may_read_for_fitting(facts, request.exclusions.as_ref()));
    let access = facts.as_ref().map_or(
        ContextAccess::Denied {
            reason: DenyReason::UnknownApp,
        },
        |facts| decide_context(true, facts, request.exclusions.as_ref()),
    );
    let allowed = access == ContextAccess::Allowed;
    if !fitting && !allowed {
        return TargetContext::default();
    }
    let Some(read) = field.read() else {
        return TargetContext::default();
    };
    let has_selection = read.selection.len > 0;
    let voice_edit = request.selection
        && allowed
        && has_selection
        && read.selected.chars().count() <= MAX_SELECTION_CHARS;
    let snapshot = (request.external && allowed)
        .then(|| {
            let window = platform::window_text(target, MAX_WINDOW_TEXT_CHARS);
            // Text read after focus moved belongs to another window.
            field.focused().then(|| {
                ContextSnapshot::collect(
                    &request.operation_id,
                    now_ms(),
                    &access,
                    facts.as_ref().unwrap_or(&WindowFacts::default()),
                    Some(&read.before),
                    window.as_deref(),
                    ordinary_words(),
                )
            })
        })
        .flatten()
        .flatten();
    TargetContext {
        before: fitting.then(|| tail_chars(&read.before, MAX_BEFORE_CHARS)),
        has_selection,
        selection: voice_edit.then(|| read.selected.clone()),
        fingerprint: voice_edit.then(|| read.fingerprint()),
        snapshot,
    }
}

fn access(target: &str, request: &ContextRequest) -> ContextAccess {
    platform::facts(target).map_or(
        ContextAccess::Denied {
            reason: DenyReason::UnknownApp,
        },
        |facts| decide_context(true, &facts, request.exclusions.as_ref()),
    )
}

fn tail_chars(text: &str, max: usize) -> String {
    let skip = text.chars().count().saturating_sub(max);
    text.chars().skip(skip).collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

fn ordinary_words() -> Option<&'static OrdinaryWords> {
    static WORDS: LazyLock<Option<OrdinaryWords>> = LazyLock::new(|| {
        std::fs::read_to_string("/usr/share/dict/words")
            .ok()
            .map(|lines| OrdinaryWords::from_lines(&lines))
    });
    WORDS.as_ref()
}

static TARGETS: LazyLock<Targets> = LazyLock::new(Targets::default);

/// The writes made into recently captured fields, and which dictation may still use each.
#[derive(Default)]
struct Targets {
    /// Newest first. Held for a whole write, so writes never interleave.
    histories: Mutex<VecDeque<(String, History)>>,
    /// Never held during a native call, so ending a field lands while a write waits for
    /// `histories` or between the steps of one.
    leases: Mutex<Leases>,
}

struct Lease {
    operation_id: String,
    account_id: String,
    /// Set by the applied delivery; until then only the delivery itself may write.
    after_until: Option<Instant>,
}

#[derive(Default)]
struct Leases {
    held: HashMap<String, Lease>,
    ended: VecDeque<String>,
}

impl Leases {
    fn is_ended(&self, target: &str) -> bool {
        self.ended.iter().any(|ended| ended == target)
    }

    fn end(&mut self, target: &str) {
        self.held.remove(target);
        if !self.is_ended(target) {
            if self.ended.len() >= ENDED_TARGETS {
                self.ended.pop_front();
            }
            self.ended.push_back(target.to_owned());
        }
    }

    /// The field takes `operation_id`'s delivery: unclaimed, or already claimed by it.
    fn claim(&mut self, target: &str, operation_id: &str, account_id: &str) -> bool {
        if self.is_ended(target) {
            return false;
        }
        let lease = self.held.entry(target.to_owned()).or_insert_with(|| Lease {
            operation_id: operation_id.to_owned(),
            account_id: account_id.to_owned(),
            after_until: None,
        });
        lease.operation_id == operation_id && lease.after_until.is_none()
    }

    fn delivering(&self, target: &str, operation_id: &str) -> bool {
        self.held
            .get(target)
            .is_some_and(|lease| lease.operation_id == operation_id && lease.after_until.is_none())
    }

    /// The window after `operation_id`'s applied delivery is still open.
    fn after(&mut self, target: &str, operation_id: &str, now: Instant) -> bool {
        match self
            .held
            .get(target)
            .map(|lease| (lease.after_until, &lease.operation_id))
        {
            Some((Some(until), id)) if now < until => id == operation_id,
            Some((Some(_), _)) => {
                self.end(target);
                false
            }
            _ => false,
        }
    }

    fn open_after(&mut self, target: &str, operation_id: &str, until: Instant) {
        if let Some(lease) = self.held.get_mut(target)
            && lease.operation_id == operation_id
        {
            lease.after_until = Some(until);
        }
    }

    fn retire(&mut self, account: Option<&str>) -> Vec<String> {
        let ended: Vec<String> = self
            .held
            .iter()
            .filter(|(_, lease)| account.is_none_or(|account| lease.account_id == account))
            .map(|(target, _)| target.clone())
            .collect();
        for target in &ended {
            self.end(target);
        }
        ended
    }
}

impl Targets {
    /// One guarded write for `request.operation_id`. `open` gives the field and whether it may
    /// be read for fitting.
    fn insert<P: FieldPort>(
        &self,
        target: &str,
        request: &InsertRequest,
        deliverable: &dyn Fn() -> Option<String>,
        open: impl FnOnce() -> Result<(P, bool), InsertMiss>,
    ) -> InsertOutcome {
        let operation = request.operation_id.as_str();
        let restore = matches!(request.mode, InsertMode::Restore { .. });
        let live = || {
            if restore {
                self.leases().after(target, operation, Instant::now())
            } else {
                deliverable().is_some() && self.leases().delivering(target, operation)
            }
        };
        let claimed = if restore {
            live()
        } else {
            deliverable().is_some_and(|account| self.leases().claim(target, operation, &account))
        };
        if !claimed {
            return InsertOutcome::Unchanged {
                reason: InsertMiss::Retired,
            };
        }
        let outcome = match open() {
            Ok((field, fitting)) => self.with_history(target, |history| {
                edit::insert(
                    &field,
                    history,
                    &request.edit_id,
                    &request.text,
                    &request.mode,
                    fitting,
                    &live,
                )
            }),
            Err(reason) => InsertOutcome::Unchanged { reason },
        };
        if !restore {
            // Only an applied delivery has anything to restore or learn from.
            let mut leases = self.leases();
            match outcome {
                InsertOutcome::Applied { .. } => {
                    leases.open_after(target, operation, Instant::now() + AFTER_DELIVERY)
                }
                _ => leases.end(target),
            }
        }
        outcome
    }

    /// The field around the caret before `edit_id` applied, while its window is open.
    fn kept_start(&self, target: &str, operation_id: &str, edit_id: &str) -> Option<FieldText> {
        if !self.leases().after(target, operation_id, Instant::now()) {
            return None;
        }
        self.with_history(target, |history| history.start_of(edit_id).cloned())
    }

    fn end(&self, target: &str) {
        self.leases().end(target);
    }

    fn retire(&self, account: Option<&str>) -> Vec<String> {
        self.leases().retire(account)
    }

    fn with_history<T>(&self, target: &str, act: impl FnOnce(&mut History) -> T) -> T {
        let mut histories = self
            .histories
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let index = match histories.iter().position(|(token, _)| token == target) {
            Some(index) => index,
            None => {
                histories.push_front((target.to_owned(), History::default()));
                histories.truncate(KEPT_TARGETS);
                0
            }
        };
        act(&mut histories[index].1)
    }

    fn forget(&self, target: &str) {
        self.histories
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|(token, _)| token != target);
    }

    fn leases(&self) -> MutexGuard<'_, Leases> {
        self.leases
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
    };

    use super::{edit::tests::Fake, *};

    const ACCOUNT: &str = "account-a";

    fn account() -> Option<String> {
        Some(ACCOUNT.into())
    }

    fn released() -> Option<String> {
        None
    }

    fn request(operation_id: &str, text: &str, mode: InsertMode) -> InsertRequest {
        InsertRequest {
            operation_id: operation_id.into(),
            edit_id: operation_id.into(),
            text: text.into(),
            mode,
            exclusions: None,
        }
    }

    fn restore(operation_id: &str) -> InsertRequest {
        InsertRequest {
            edit_id: format!("{operation_id}:raw"),
            ..request(
                operation_id,
                ", um, if you want it",
                InsertMode::Restore {
                    edit_id: "op-1".into(),
                },
            )
        }
    }

    fn retired() -> InsertOutcome {
        InsertOutcome::Unchanged {
            reason: InsertMiss::Retired,
        }
    }

    #[test]
    fn a_write_queued_behind_another_is_never_made_once_its_dictation_ends() {
        for end in ["cancel", "release", "sign-out"] {
            let targets = Arc::new(Targets::default());
            // The first write holds the field lock until it is let through.
            let (entered_tx, entered) = mpsc::channel();
            let (let_through, waiting) = mpsc::channel();
            let first = thread::spawn({
                let targets = targets.clone();
                move || {
                    let field = Fake::gated("One", entered_tx, waiting);
                    let outcome = targets.insert(
                        "first",
                        &request("op-1", " two", InsertMode::Caret),
                        &account,
                        || Ok((&field, true)),
                    );
                    (outcome, field.text())
                }
            });
            entered.recv().unwrap();

            let deliverable = Arc::new(AtomicBool::new(true));
            let (queued_tx, queued) = mpsc::channel();
            let second = thread::spawn({
                let (targets, deliverable) = (targets.clone(), deliverable.clone());
                move || {
                    let field = Fake::at_end("Hello");
                    let outcome = targets.insert(
                        "second",
                        &request("op-2", " world", InsertMode::Caret),
                        &|| deliverable.load(Ordering::SeqCst).then(|| ACCOUNT.into()),
                        || {
                            queued_tx.send(()).unwrap();
                            Ok((&field, true))
                        },
                    );
                    (outcome, field.text())
                }
            });
            // The second write is claimed and waits for the field lock.
            queued.recv().unwrap();
            match end {
                "cancel" => deliverable.store(false, Ordering::SeqCst),
                "release" => targets.end("second"),
                _ => {
                    targets.retire(Some(ACCOUNT));
                }
            }
            let_through.send(()).unwrap();

            let (outcome, text) = second.join().unwrap();
            assert_eq!(outcome, retired(), "{end}");
            assert_eq!(text, "Hello", "{end}");
            // The first write was already sent when its account signed out: it is read back.
            let (outcome, text) = first.join().unwrap();
            assert!(matches!(outcome, InsertOutcome::Applied { .. }), "{end}");
            assert_eq!(text, "One two", "{end}");
        }
    }

    #[test]
    fn a_delivered_field_serves_only_its_own_restore_and_learning_window() {
        let targets = Targets::default();
        let field = Fake::at_end("It works.");
        let open = || Ok::<_, InsertMiss>((&field, true));
        let delivery = request(
            "op-1",
            ", if you want it",
            InsertMode::Fit { can_join: true },
        );
        assert!(matches!(
            targets.insert("t", &delivery, &account, open),
            InsertOutcome::Applied { .. }
        ));
        // One delivery per dictation; the engine has already released the operation since.
        assert_eq!(
            targets.insert(
                "t",
                &request("op-1", " again", InsertMode::Caret),
                &account,
                open
            ),
            retired()
        );
        // Only the dictation that delivered may restore into or read back its field.
        assert_eq!(
            targets.insert("t", &restore("op-2"), &released, open),
            retired()
        );
        assert!(targets.kept_start("t", "op-2", "op-1").is_none());
        assert!(targets.kept_start("t", "op-1", "op-1").is_some());
        targets.retire(Some("account-b"));
        assert!(matches!(
            targets.insert("t", &restore("op-1"), &released, open),
            InsertOutcome::Applied { .. }
        ));
        assert_eq!(field.text(), "It works. Um, if you want it");

        for end in ["release", "sign-out", "meeting", "expiry"] {
            let targets = Targets::default();
            let field = Fake::at_end("It works.");
            let open = || Ok::<_, InsertMiss>((&field, true));
            targets.insert("t", &delivery, &account, open);
            match end {
                "release" => targets.end("t"),
                "sign-out" => {
                    targets.retire(Some(ACCOUNT));
                }
                "meeting" => {
                    targets.retire(None);
                }
                _ => {
                    let mut leases = targets.leases();
                    leases.held.get_mut("t").unwrap().after_until = Some(Instant::now());
                }
            }
            assert_eq!(
                targets.insert("t", &restore("op-1"), &released, open),
                retired(),
                "{end}"
            );
            assert!(targets.kept_start("t", "op-1", "op-1").is_none(), "{end}");
            assert_eq!(field.text(), "It works, if you want it", "{end}");
        }
    }
}
