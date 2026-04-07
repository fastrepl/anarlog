use std::{
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    thread,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use futures_core::Stream;
use url::{Host, Url};

mod types;

pub use types::*;

pub type CaptureStream =
    Pin<Box<dyn Stream<Item = Result<Transition, CaptureError>> + Send + 'static>>;

impl CaptureAccess {
    pub fn allows_snapshot(self) -> bool {
        self != Self::None
    }

    pub fn allows_url(self) -> bool {
        matches!(self, Self::Url | Self::Full)
    }

    pub fn allows_text(self) -> bool {
        self == Self::Full
    }
}

impl PolicyMode {
    pub fn default_access(self) -> CaptureAccess {
        match self {
            Self::OptIn => CaptureAccess::Metadata,
            Self::OptOut => CaptureAccess::Full,
        }
    }
}

impl DomainRule {
    pub fn matches_host(&self, host: &str) -> bool {
        let domain = normalize_domain(&self.domain);
        let host = normalize_domain(host);
        if domain.is_empty() || host.is_empty() {
            return false;
        }

        host == domain || (self.include_subdomains && host.ends_with(&format!(".{domain}")))
    }
}

impl BrowserPolicy {
    pub fn access_for_host(&self, host: Option<&str>, mode: PolicyMode) -> CaptureAccess {
        let Some(host) = host else {
            return mode.default_access();
        };

        let mut matched = None;
        for rule in &self.rules {
            if !rule.matches_host(host) {
                continue;
            }
            if rule.access == CaptureAccess::None {
                return CaptureAccess::None;
            }
            matched = Some(rule.access);
        }

        matched.unwrap_or_else(|| mode.default_access())
    }

    pub fn sanitize_url(&self, raw_url: &str) -> Option<SanitizedBrowserUrl> {
        let mut parsed = Url::parse(raw_url).ok()?;
        let host = parsed
            .host()
            .and_then(normalize_host)
            .map(normalize_domain)
            .filter(|value| !value.is_empty());

        if self.strip_query {
            parsed.set_query(None);
        }
        if self.strip_fragment {
            parsed.set_fragment(None);
        }

        Some(SanitizedBrowserUrl {
            url: parsed.to_string(),
            host,
        })
    }

    pub fn access_for_context(
        &self,
        browser: &BrowserContext,
        mode: PolicyMode,
    ) -> CaptureDecision {
        if self.block_private_browsing && browser.is_private {
            return CaptureDecision {
                access: CaptureAccess::None,
                skip: true,
                source: SnapshotSource::Workspace,
                activity_kind: ActivityKind::Browser,
                url: None,
            };
        }

        let sanitized = browser
            .raw_url
            .as_deref()
            .and_then(|value| self.sanitize_url(value));
        let has_sanitized_url = sanitized.is_some();
        let mut access = self.access_for_host(
            sanitized.as_ref().and_then(|value| value.host.as_deref()),
            mode,
        );
        if !has_sanitized_url && access > CaptureAccess::Metadata {
            access = CaptureAccess::Metadata;
        }
        if !has_sanitized_url && access.allows_text() && self.require_url_for_text_access {
            access = CaptureAccess::Metadata;
        }

        CaptureDecision {
            access,
            skip: !access.allows_snapshot(),
            source: source_for_access(access, SnapshotSource::Accessibility),
            activity_kind: ActivityKind::Browser,
            url: access
                .allows_url()
                .then(|| sanitized.map(|value| value.url))
                .flatten(),
        }
    }
}

impl Default for BrowserPolicy {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            require_url_for_text_access: true,
            block_private_browsing: true,
            strip_query: true,
            strip_fragment: true,
        }
    }
}

impl CapturePolicy {
    pub fn access_for_bundle(&self, bundle_id: Option<&str>) -> CaptureAccess {
        let Some(bundle_id) = bundle_id.map(str::trim).filter(|value| !value.is_empty()) else {
            return self.mode.default_access();
        };

        let mut matched = None;
        for rule in &self.app_rules {
            if rule.bundle_id != bundle_id {
                continue;
            }
            if rule.access == CaptureAccess::None {
                return CaptureAccess::None;
            }
            matched = Some(rule.access);
        }

        matched.unwrap_or_else(|| self.mode.default_access())
    }

    pub fn access_for_app(&self, app: &AppIdentity) -> CaptureAccess {
        let ids = [
            app.bundle_id.as_deref(),
            Some(app.app_id.as_str()),
            app.executable_path.as_deref(),
        ];

        let mut matched = None;
        for candidate in ids.into_iter().flatten() {
            let candidate = candidate.trim();
            if candidate.is_empty() {
                continue;
            }

            for rule in &self.app_rules {
                if rule.bundle_id != candidate {
                    continue;
                }
                if rule.access == CaptureAccess::None {
                    return CaptureAccess::None;
                }
                matched = Some(rule.access);
            }
        }

        matched.unwrap_or_else(|| self.mode.default_access())
    }

    pub fn decision_for_candidate(&self, candidate: &CaptureCandidate) -> CaptureDecision {
        let app_access = self.access_for_app(&candidate.app);
        if !app_access.allows_snapshot() {
            return CaptureDecision {
                access: app_access,
                skip: true,
                source: SnapshotSource::Workspace,
                activity_kind: candidate.activity_kind,
                url: None,
            };
        }

        if let Some(browser) = &candidate.browser {
            let browser_decision = self.browser.access_for_context(browser, self.mode);
            let access = std::cmp::min(app_access, browser_decision.access);
            return CaptureDecision {
                access,
                skip: !access.allows_snapshot() || browser_decision.skip,
                source: source_for_access(access, candidate.source),
                activity_kind: ActivityKind::Browser,
                url: access
                    .allows_url()
                    .then_some(browser_decision.url)
                    .flatten(),
            };
        }

        let access = normalize_non_browser_access(app_access);
        CaptureDecision {
            access,
            skip: !access.allows_snapshot(),
            source: source_for_access(access, candidate.source),
            activity_kind: candidate.activity_kind,
            url: None,
        }
    }
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            mode: PolicyMode::OptIn,
            app_rules: Vec::new(),
            browser: BrowserPolicy::default(),
        }
    }
}

impl Snapshot {
    pub fn from_spec(spec: SnapshotSpec) -> Self {
        let content_level = content_level_for_access(spec.access);
        let text_anchor = spec
            .access
            .allows_text()
            .then_some(spec.text_anchor)
            .flatten();

        Self {
            app: spec.app.clone(),
            activity_kind: spec.activity_kind,
            captured_at: spec.captured_at,
            pid: spec.app.pid,
            app_name: spec.app.app_name.clone(),
            bundle_id: spec.app.bundle_id.clone(),
            window_title: spec
                .access
                .allows_text()
                .then_some(spec.window_title)
                .flatten(),
            url: spec.access.allows_url().then_some(spec.url).flatten(),
            visible_text: spec
                .access
                .allows_text()
                .then_some(spec.visible_text)
                .flatten(),
            text_anchor_kind: text_anchor.as_ref().map(|value| value.kind),
            text_anchor_identity: text_anchor.as_ref().map(|value| value.identity.clone()),
            text_anchor_text: text_anchor.as_ref().and_then(|value| value.text.clone()),
            text_anchor_prefix: text_anchor.as_ref().and_then(|value| value.prefix.clone()),
            text_anchor_suffix: text_anchor.as_ref().and_then(|value| value.suffix.clone()),
            text_anchor_selected_text: text_anchor
                .as_ref()
                .and_then(|value| value.selected_text.clone()),
            text_anchor_confidence: text_anchor.as_ref().map(|value| value.confidence),
            content_level,
            source: source_for_access(spec.access, spec.source),
        }
    }

    pub fn fingerprint(&self) -> String {
        let content_level = match self.content_level {
            ContentLevel::Metadata => "metadata",
            ContentLevel::Url => "url",
            ContentLevel::Full => "full",
        };
        let has_anchor = self.text_anchor_identity.is_some()
            || self.text_anchor_text.is_some()
            || self.text_anchor_selected_text.is_some();
        let ambient_text = if has_anchor {
            ""
        } else {
            self.visible_text.as_deref().unwrap_or_default()
        };

        STANDARD_NO_PAD.encode(
            [
                content_level,
                self.app.app_id.as_str(),
                match self.activity_kind {
                    ActivityKind::ForegroundWindow => "foreground_window",
                    ActivityKind::Browser => "browser",
                    ActivityKind::AudioSession => "audio_session",
                },
                self.window_title.as_deref().unwrap_or_default(),
                self.url.as_deref().unwrap_or_default(),
                self.text_anchor_identity.as_deref().unwrap_or_default(),
                self.text_anchor_text.as_deref().unwrap_or_default(),
                self.text_anchor_selected_text
                    .as_deref()
                    .unwrap_or_default(),
                ambient_text,
            ]
            .join("|"),
        )
    }
}

impl Event {
    pub fn from_snapshot(snapshot: Snapshot) -> Self {
        Self {
            started_at: snapshot.captured_at,
            ended_at: snapshot.captured_at,
            fingerprint: snapshot.fingerprint(),
            snapshot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOptions {
    pub poll_interval: Duration,
    pub emit_initial: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            emit_initial: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureErrorKind {
    PermissionDenied,
    Unsupported,
    TemporarilyUnavailable,
    Platform,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct CaptureError {
    pub kind: CaptureErrorKind,
    pub message: String,
}

impl CaptureError {
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(CaptureErrorKind::PermissionDenied, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(CaptureErrorKind::Unsupported, message)
    }

    pub fn temporarily_unavailable(message: impl Into<String>) -> Self {
        Self::new(CaptureErrorKind::TemporarilyUnavailable, message)
    }

    pub fn platform(message: impl Into<String>) -> Self {
        Self::new(CaptureErrorKind::Platform, message)
    }

    pub fn new(kind: CaptureErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub trait ActivityCapture: Send + Sync {
    fn capabilities(&self) -> Capabilities;

    fn snapshot(&self) -> Result<Option<Snapshot>, CaptureError>;

    fn watch(&self, options: WatchOptions) -> Result<CaptureStream, CaptureError>;
}

pub fn spawn_polling_watch_stream<F>(
    thread_name: impl Into<String>,
    poll_snapshot: F,
    options: WatchOptions,
) -> Result<CaptureStream, CaptureError>
where
    F: FnMut() -> Result<Option<Snapshot>, CaptureError> + Send + 'static,
{
    let (transition_tx, transition_rx) = tokio::sync::mpsc::unbounded_channel();
    let stop = Arc::new(StopSignal::default());
    let thread_stop = Arc::clone(&stop);

    let handle = thread::Builder::new()
        .name(thread_name.into())
        .spawn(move || watch_loop(poll_snapshot, options, thread_stop, transition_tx))
        .map_err(|error| CaptureError::platform(error.to_string()))?;

    Ok(Box::pin(WatchStream {
        inner: tokio_stream::wrappers::UnboundedReceiverStream::new(transition_rx),
        stop,
        handle: Some(handle),
    }))
}

fn normalize_domain(value: &str) -> String {
    value.trim().trim_matches('.').to_ascii_lowercase()
}

fn normalize_host(host: Host<&str>) -> Option<&str> {
    match host {
        Host::Domain(value) => Some(value),
        _ => None,
    }
}

fn normalize_non_browser_access(access: CaptureAccess) -> CaptureAccess {
    match access {
        CaptureAccess::Url => CaptureAccess::Metadata,
        _ => access,
    }
}

pub fn content_level_for_access(access: CaptureAccess) -> ContentLevel {
    match access {
        CaptureAccess::Metadata | CaptureAccess::None => ContentLevel::Metadata,
        CaptureAccess::Url => ContentLevel::Url,
        CaptureAccess::Full => ContentLevel::Full,
    }
}

pub fn source_for_access(access: CaptureAccess, preferred: SnapshotSource) -> SnapshotSource {
    if access == CaptureAccess::Metadata {
        SnapshotSource::Workspace
    } else {
        preferred
    }
}

#[derive(Debug, Default, Clone)]
pub struct EventCoalescer {
    current: Option<Event>,
    current_suppressed_snapshot_count: u32,
    sequence: u64,
}

impl EventCoalescer {
    pub fn current(&self) -> Option<&Event> {
        self.current.as_ref()
    }

    pub fn push(&mut self, snapshot: Option<Snapshot>) -> Option<Transition> {
        match (self.current.take(), snapshot) {
            (None, None) => None,
            (None, Some(snapshot)) => {
                let current = Event::from_snapshot(snapshot);
                self.current = Some(current.clone());
                self.current_suppressed_snapshot_count = 0;
                self.sequence += 1;
                Some(Transition {
                    previous: None,
                    current: Some(current),
                    reason: TransitionReason::Started,
                    sequence: self.sequence,
                    suppressed_snapshot_count: 0,
                })
            }
            (Some(previous), None) => {
                let suppressed_snapshot_count = self.current_suppressed_snapshot_count;
                self.current_suppressed_snapshot_count = 0;
                self.sequence += 1;
                Some(Transition {
                    previous: Some(previous),
                    current: None,
                    reason: TransitionReason::Idle,
                    sequence: self.sequence,
                    suppressed_snapshot_count,
                })
            }
            (Some(mut current), Some(snapshot)) => {
                let fingerprint = snapshot.fingerprint();
                if current.fingerprint == fingerprint {
                    current.ended_at = snapshot.captured_at;
                    current.snapshot = snapshot;
                    self.current = Some(current);
                    self.current_suppressed_snapshot_count += 1;
                    None
                } else {
                    let next = Event::from_snapshot(snapshot);
                    let reason = transition_reason(&current.snapshot, &next.snapshot);
                    let suppressed_snapshot_count = self.current_suppressed_snapshot_count;
                    self.current = Some(next.clone());
                    self.current_suppressed_snapshot_count = 0;
                    self.sequence += 1;
                    Some(Transition {
                        previous: Some(current),
                        current: Some(next),
                        reason,
                        sequence: self.sequence,
                        suppressed_snapshot_count,
                    })
                }
            }
        }
    }
}

fn transition_reason(previous: &Snapshot, current: &Snapshot) -> TransitionReason {
    if previous.app.app_id != current.app.app_id {
        return TransitionReason::AppChanged;
    }

    if previous.activity_kind != current.activity_kind {
        return TransitionReason::ActivityKindChanged;
    }

    if previous.url != current.url {
        return TransitionReason::UrlChanged;
    }

    if previous.window_title != current.window_title {
        return TransitionReason::TitleChanged;
    }

    if previous.text_anchor_kind != current.text_anchor_kind
        || previous.text_anchor_identity != current.text_anchor_identity
    {
        return TransitionReason::TextAnchorChanged;
    }

    TransitionReason::ContentChanged
}

fn watch_loop<F>(
    mut poll_snapshot: F,
    options: WatchOptions,
    stop: Arc<StopSignal>,
    transition_tx: tokio::sync::mpsc::UnboundedSender<Result<Transition, CaptureError>>,
) where
    F: FnMut() -> Result<Option<Snapshot>, CaptureError>,
{
    let mut state = WatchState::new(options);
    let mut first_iteration = true;

    loop {
        if !first_iteration && stop.wait_timeout(options.poll_interval) {
            break;
        }
        first_iteration = false;

        if stop.is_set() {
            break;
        }

        match poll_snapshot() {
            Ok(snapshot) => {
                let Some(transition) = state.push(snapshot) else {
                    continue;
                };

                if transition_tx.send(Ok(transition)).is_err() {
                    break;
                }
            }
            Err(error) => {
                let _ = transition_tx.send(Err(error));
                break;
            }
        }
    }
}

#[derive(Default)]
struct StopSignal {
    stopped: AtomicBool,
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl StopSignal {
    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.condvar.notify_all();
    }

    fn is_set(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    fn wait_timeout(&self, duration: Duration) -> bool {
        if self.is_set() {
            return true;
        }

        let guard = self.mutex.lock().unwrap_or_else(|error| error.into_inner());
        let result = self
            .condvar
            .wait_timeout_while(guard, duration, |_| !self.is_set());
        match result {
            Ok((_, _)) => self.is_set(),
            Err(error) => {
                let _ = error.into_inner();
                self.is_set()
            }
        }
    }
}

struct WatchState {
    coalescer: EventCoalescer,
    first_transition_suppressed: bool,
}

impl WatchState {
    fn new(options: WatchOptions) -> Self {
        Self {
            coalescer: EventCoalescer::default(),
            first_transition_suppressed: !options.emit_initial,
        }
    }

    fn push(&mut self, snapshot: Option<Snapshot>) -> Option<Transition> {
        let transition = self.coalescer.push(snapshot)?;
        if self.first_transition_suppressed
            && transition.previous.is_none()
            && transition.current.is_some()
        {
            self.first_transition_suppressed = false;
            return None;
        }

        self.first_transition_suppressed = false;
        Some(transition)
    }
}

struct WatchStream {
    inner: tokio_stream::wrappers::UnboundedReceiverStream<Result<Transition, CaptureError>>,
    stop: Arc<StopSignal>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Stream for WatchStream {
    type Item = Result<Transition, CaptureError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for WatchStream {
    fn drop(&mut self) {
        self.stop.stop();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn app_identity() -> AppIdentity {
        AppIdentity {
            pid: 42,
            app_name: "TextEdit".to_string(),
            app_id: "com.apple.TextEdit".to_string(),
            app_id_kind: AppIdKind::BundleId,
            bundle_id: Some("com.apple.TextEdit".to_string()),
            executable_path: None,
        }
    }

    fn snapshot(title: &str) -> Snapshot {
        Snapshot {
            app: app_identity(),
            activity_kind: ActivityKind::ForegroundWindow,
            captured_at: SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            pid: 42,
            app_name: "TextEdit".to_string(),
            bundle_id: Some("com.apple.TextEdit".to_string()),
            window_title: Some(title.to_string()),
            url: None,
            visible_text: Some("hello".to_string()),
            text_anchor_kind: Some(TextAnchorKind::FocusedEdit),
            text_anchor_identity: Some("editor:notes".to_string()),
            text_anchor_text: Some("hello".to_string()),
            text_anchor_prefix: None,
            text_anchor_suffix: None,
            text_anchor_selected_text: None,
            text_anchor_confidence: Some(TextAnchorConfidence::High),
            content_level: ContentLevel::Full,
            source: SnapshotSource::Accessibility,
        }
    }

    #[test]
    fn fingerprint_is_stable() {
        let left = snapshot("Notes");
        let right = snapshot("Notes");

        assert_eq!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn coalescer_emits_initial_transition() {
        let mut coalescer = EventCoalescer::default();
        let transition = coalescer.push(Some(snapshot("Notes"))).unwrap();

        assert!(transition.previous.is_none());
        assert_eq!(transition.reason, TransitionReason::Started);
        assert_eq!(transition.sequence, 1);
        assert_eq!(transition.suppressed_snapshot_count, 0);
        assert_eq!(
            transition.current.unwrap().snapshot.window_title.as_deref(),
            Some("Notes")
        );
    }

    #[test]
    fn coalescer_suppresses_extensions() {
        let mut coalescer = EventCoalescer::default();
        let _ = coalescer.push(Some(snapshot("Notes")));

        let mut same = snapshot("Notes");
        same.captured_at += Duration::from_secs(5);

        assert!(coalescer.push(Some(same)).is_none());
        assert_eq!(
            coalescer.current().unwrap().ended_at,
            SystemTime::UNIX_EPOCH + Duration::from_secs(15)
        );
    }

    #[test]
    fn coalescer_emits_change_transition() {
        let mut coalescer = EventCoalescer::default();
        let _ = coalescer.push(Some(snapshot("Notes")));
        let mut same = snapshot("Notes");
        same.captured_at += Duration::from_secs(5);
        let _ = coalescer.push(Some(same));
        let transition = coalescer.push(Some(snapshot("Docs"))).unwrap();

        assert_eq!(transition.reason, TransitionReason::TitleChanged);
        assert_eq!(transition.sequence, 2);
        assert_eq!(transition.suppressed_snapshot_count, 1);
        assert_eq!(
            transition
                .previous
                .unwrap()
                .snapshot
                .window_title
                .as_deref(),
            Some("Notes")
        );
        assert_eq!(
            transition.current.unwrap().snapshot.window_title.as_deref(),
            Some("Docs")
        );
    }

    #[test]
    fn coalescer_emits_idle_transition() {
        let mut coalescer = EventCoalescer::default();
        let _ = coalescer.push(Some(snapshot("Notes")));
        let mut same = snapshot("Notes");
        same.captured_at += Duration::from_secs(5);
        let _ = coalescer.push(Some(same));
        let transition = coalescer.push(None).unwrap();

        assert_eq!(transition.reason, TransitionReason::Idle);
        assert_eq!(transition.sequence, 2);
        assert_eq!(transition.suppressed_snapshot_count, 1);
        assert_eq!(
            transition
                .previous
                .unwrap()
                .snapshot
                .window_title
                .as_deref(),
            Some("Notes")
        );
        assert!(transition.current.is_none());
    }

    #[test]
    fn fingerprint_prefers_anchor_text_over_ambient_text() {
        let mut left = snapshot("Notes");
        let mut right = snapshot("Notes");

        left.visible_text = Some("ambient one".to_string());
        right.visible_text = Some("ambient two".to_string());

        assert_eq!(left.fingerprint(), right.fingerprint());

        right.text_anchor_text = Some("changed".to_string());

        assert_ne!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn coalescer_detects_app_change() {
        let mut coalescer = EventCoalescer::default();
        let _ = coalescer.push(Some(snapshot("Notes")));

        let mut next = snapshot("Notes");
        next.app.app_id = "com.google.Chrome".to_string();
        next.app_name = "Google Chrome".to_string();
        next.bundle_id = Some("com.google.Chrome".to_string());

        let transition = coalescer.push(Some(next)).unwrap();

        assert_eq!(transition.reason, TransitionReason::AppChanged);
    }

    #[test]
    fn fingerprint_falls_back_to_visible_text_when_anchor_is_missing() {
        let mut left = snapshot("Notes");
        let mut right = snapshot("Notes");

        left.text_anchor_kind = None;
        left.text_anchor_identity = None;
        left.text_anchor_text = None;
        left.text_anchor_prefix = None;
        left.text_anchor_suffix = None;
        left.text_anchor_selected_text = None;
        left.text_anchor_confidence = None;

        right.text_anchor_kind = None;
        right.text_anchor_identity = None;
        right.text_anchor_text = None;
        right.text_anchor_prefix = None;
        right.text_anchor_suffix = None;
        right.text_anchor_selected_text = None;
        right.text_anchor_confidence = None;
        right.visible_text = Some("different".to_string());

        assert_ne!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn domain_rule_matches_subdomains_when_enabled() {
        let rule = DomainRule {
            domain: "example.com".to_string(),
            include_subdomains: true,
            access: CaptureAccess::Full,
        };

        assert!(rule.matches_host("example.com"));
        assert!(rule.matches_host("docs.example.com"));
        assert!(!rule.matches_host("otherexample.com"));
    }

    #[test]
    fn browser_policy_uses_last_matching_rule() {
        let policy = BrowserPolicy {
            rules: vec![
                DomainRule {
                    domain: "example.com".to_string(),
                    include_subdomains: true,
                    access: CaptureAccess::Url,
                },
                DomainRule {
                    domain: "docs.example.com".to_string(),
                    include_subdomains: false,
                    access: CaptureAccess::Full,
                },
            ],
            ..Default::default()
        };

        assert_eq!(
            policy.access_for_host(Some("docs.example.com"), PolicyMode::OptIn),
            CaptureAccess::Full
        );
        assert_eq!(
            policy.access_for_host(Some("www.example.com"), PolicyMode::OptIn),
            CaptureAccess::Url
        );
        assert_eq!(
            policy.access_for_host(Some("other.com"), PolicyMode::OptIn),
            CaptureAccess::Metadata
        );
    }

    #[test]
    fn capture_policy_denies_when_any_matching_rule_denies() {
        let policy = CapturePolicy {
            mode: PolicyMode::OptIn,
            app_rules: vec![
                BundleRule {
                    bundle_id: "com.example.app".to_string(),
                    access: CaptureAccess::None,
                },
                BundleRule {
                    bundle_id: "com.example.app".to_string(),
                    access: CaptureAccess::Full,
                },
            ],
            browser: BrowserPolicy::default(),
        };

        assert_eq!(
            policy.access_for_bundle(Some("com.example.app")),
            CaptureAccess::None
        );
        assert_eq!(
            policy.access_for_bundle(Some("com.example.other")),
            CaptureAccess::Metadata
        );
    }

    #[test]
    fn capture_policy_matches_executable_path_when_bundle_id_is_missing() {
        let policy = CapturePolicy {
            mode: PolicyMode::OptIn,
            app_rules: vec![BundleRule {
                bundle_id: "C:\\Program Files\\Slack\\slack.exe".to_string(),
                access: CaptureAccess::Full,
            }],
            browser: BrowserPolicy::default(),
        };
        let app = AppIdentity {
            pid: 7,
            app_name: "slack.exe".to_string(),
            app_id: "C:\\Program Files\\Slack\\slack.exe".to_string(),
            app_id_kind: AppIdKind::ExecutablePath,
            bundle_id: None,
            executable_path: Some("C:\\Program Files\\Slack\\slack.exe".to_string()),
        };

        assert_eq!(policy.access_for_app(&app), CaptureAccess::Full);
    }

    #[test]
    fn browser_policy_sanitizes_url_and_decides_access() {
        let policy = BrowserPolicy {
            rules: vec![DomainRule {
                domain: "example.com".to_string(),
                include_subdomains: true,
                access: CaptureAccess::Full,
            }],
            ..Default::default()
        };

        let decision = policy.access_for_context(
            &BrowserContext {
                raw_url: Some("https://docs.example.com/path?q=1#anchor".to_string()),
                is_private: false,
            },
            PolicyMode::OptIn,
        );

        assert_eq!(decision.access, CaptureAccess::Full);
        assert_eq!(
            decision.url.as_deref(),
            Some("https://docs.example.com/path")
        );
        assert_eq!(decision.activity_kind, ActivityKind::Browser);
    }

    #[test]
    fn capture_policy_decision_normalizes_url_access_for_non_browser_activity() {
        let policy = CapturePolicy {
            mode: PolicyMode::OptIn,
            app_rules: vec![BundleRule {
                bundle_id: "com.example.editor".to_string(),
                access: CaptureAccess::Url,
            }],
            browser: BrowserPolicy::default(),
        };
        let decision = policy.decision_for_candidate(&CaptureCandidate {
            app: AppIdentity {
                pid: 5,
                app_name: "Editor".to_string(),
                app_id: "com.example.editor".to_string(),
                app_id_kind: AppIdKind::BundleId,
                bundle_id: Some("com.example.editor".to_string()),
                executable_path: None,
            },
            activity_kind: ActivityKind::ForegroundWindow,
            source: SnapshotSource::Accessibility,
            browser: None,
        });

        assert_eq!(decision.access, CaptureAccess::Metadata);
        assert_eq!(decision.source, SnapshotSource::Workspace);
    }

    #[test]
    fn opt_out_policy_defaults_to_full() {
        let policy = CapturePolicy {
            mode: PolicyMode::OptOut,
            ..Default::default()
        };

        assert_eq!(
            policy.access_for_bundle(Some("com.example.app")),
            CaptureAccess::Full
        );
        assert_eq!(
            policy
                .browser
                .access_for_host(Some("example.com"), policy.mode),
            CaptureAccess::Full
        );
    }
}
