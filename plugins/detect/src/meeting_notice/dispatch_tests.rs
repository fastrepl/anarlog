use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
};

use anlg_audio::meeting::{CallPlatform, ConfirmedCall};
use tokio::sync::Notify;

use super::*;

const ZOOM: &str = "us.zoom.xos";

/// How the fake app takes a post.
#[derive(Clone, Copy)]
enum Answer {
    Now(Outcome),
    /// Still finding the chat until the capture ends; then answers this.
    OnCancel(Outcome),
    /// Never answers, as when the app quits after Return.
    Never,
    Refused,
    Lost,
}

struct Host {
    enabled: AtomicBool,
    answer: Mutex<Answer>,
    /// Holds the settle until released, when set.
    hold_settle: Option<Arc<Notify>>,
    posts: Mutex<Vec<NoticeTarget>>,
    /// Whether the claim was on disk when each post began.
    claimed_at_post: Mutex<Vec<bool>>,
    pool: sqlx::SqlitePool,
    /// Runs when a post or an attempt begins, for a change that lands then.
    at_post: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// What each attempt of the discovering sender answers, in order; `Sent` once none are left.
    discovered: Mutex<VecDeque<Outcome>>,
    /// Whether the claim was on disk when each discovering attempt ran.
    attempts: Mutex<Vec<bool>>,
    /// Holds the wait between discovering attempts until released, when set.
    hold_retry: Option<Arc<Notify>>,
}

impl Host {
    async fn claimed(&self) -> bool {
        let claims: i64 = sqlx::query_scalar("SELECT count(*) FROM local_meeting_notice_claims")
            .fetch_one(&self.pool)
            .await
            .unwrap();
        claims > 0
    }
}

impl NoticeHost for Arc<Host> {
    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn message(&self) -> String {
        "I'm using Anarlog to record and transcribe this meeting.".into()
    }

    fn now_ms(&self) -> i64 {
        1_800_000_000_000
    }

    async fn settle(&self) {
        if let Some(hold) = &self.hold_settle {
            hold.notified().await;
        }
    }

    async fn post(
        &self,
        target: NoticeTarget,
        _message: String,
        cancel: CancellationToken,
    ) -> PostResult {
        let claimed = self.claimed().await;
        lock(&self.claimed_at_post).push(claimed);
        lock(&self.posts).push(target);
        if let Some(change) = lock(&self.at_post).take() {
            change();
        }
        let answer = *lock(&self.answer);
        match answer {
            Answer::Now(outcome) => PostResult::Posted(outcome),
            Answer::OnCancel(outcome) => {
                cancel.cancelled().await;
                PostResult::Posted(outcome)
            }
            Answer::Never => std::future::pending().await,
            Answer::Refused => PostResult::Refused("permission_required"),
            Answer::Lost => PostResult::Lost,
        }
    }

    async fn post_discovered(&self, _message: String, wanted: StillWanted) -> DiscoveredPost {
        if let Some(change) = lock(&self.at_post).take() {
            change();
        }
        if !wanted() {
            return DiscoveredPost {
                result: PostResult::Posted(Outcome::Cancelled),
                platform: "unknown",
            };
        }
        let claimed = self.claimed().await;
        lock(&self.attempts).push(claimed);
        let outcome = lock(&self.discovered).pop_front().unwrap_or(Outcome::Sent);
        DiscoveredPost {
            result: PostResult::Posted(outcome),
            platform: "slack_huddle",
        }
    }

    async fn retry_wait(&self) {
        if let Some(hold) = &self.hold_retry {
            hold.notified().await;
        }
    }
}

struct Authority {
    current: AtomicBool,
    call: Mutex<Option<ConfirmedCall>>,
    extension: AtomicBool,
}

impl Authority {
    fn with_call(bundle_id: &str) -> Arc<Self> {
        Arc::new(Self {
            current: AtomicBool::new(true),
            call: Mutex::new(Some(ConfirmedCall {
                pid: 11,
                bundle_id: bundle_id.into(),
                platform: CallPlatform::Zoom,
                call_url: None,
            })),
            extension: AtomicBool::new(false),
        })
    }

    fn without_call() -> Arc<Self> {
        let authority = Self::with_call(ZOOM);
        *lock(&authority.call) = None;
        authority
    }
}

impl CaptureAuthority for Authority {
    fn is_current(&self, _capture: &CaptureIdentity) -> bool {
        self.current.load(Ordering::SeqCst)
    }

    fn confirmed_call(&self, _capture: &CaptureIdentity) -> Option<ConfirmedCall> {
        lock(&self.call).clone()
    }

    fn extension_handles_actions(&self, _capture: &CaptureIdentity) -> bool {
        self.extension.load(Ordering::SeqCst)
    }
}

type Dispatcher = NoticeDispatcher<Arc<Host>, sqlx::SqlitePool>;

/// An app run over a database, which a later run (a restart) opens again.
struct Run {
    host: Arc<Host>,
    dispatcher: Arc<Dispatcher>,
    _db: anlg_db_core::Db,
}

async fn open(path: &std::path::Path) -> anlg_db_core::Db {
    let db = anlg_db_core::Db::open(anlg_db_core::DbOpenOptions {
        storage: anlg_db_core::DbStorage::Local(path),
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    db
}

async fn run_with(path: &std::path::Path, configure: impl FnOnce(&mut Host)) -> Run {
    let db = open(path).await;
    let mut host = Host {
        enabled: AtomicBool::new(true),
        answer: Mutex::new(Answer::Now(Outcome::Sent)),
        hold_settle: None,
        posts: Mutex::default(),
        claimed_at_post: Mutex::default(),
        pool: db.pool().clone(),
        at_post: Mutex::new(None),
        discovered: Mutex::default(),
        attempts: Mutex::default(),
        hold_retry: None,
    };
    configure(&mut host);
    let host = Arc::new(host);
    let dispatcher = Arc::new(NoticeDispatcher::new(host.clone(), db.pool().clone()));
    Run {
        host,
        dispatcher,
        _db: db,
    }
}

fn capture(session: &str, capture: &str) -> CaptureIdentity {
    CaptureIdentity {
        account_id: "account-a".into(),
        session_id: session.into(),
        capture_id: capture.into(),
    }
}

impl Run {
    /// Starts a capture and waits for its notice to end.
    async fn record(
        &self,
        capture: CaptureIdentity,
        authority: Arc<Authority>,
    ) -> Option<NoticeReport> {
        match self.dispatcher.start(capture, authority) {
            Some(work) => work.await,
            None => None,
        }
    }

    fn posts(&self) -> usize {
        lock(&self.host.posts).len()
    }

    fn attempts(&self) -> usize {
        lock(&self.host.attempts).len()
    }

    async fn claims(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM local_meeting_notice_claims")
            .fetch_one(&self.host.pool)
            .await
            .unwrap()
    }

    async fn deliveries(&self) -> Vec<(String, String)> {
        sqlx::query_as("SELECT delivery, reason FROM session_disclosure_deliveries ORDER BY rowid")
            .fetch_all(&self.host.pool)
            .await
            .unwrap()
    }

    async fn platforms(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT platform FROM session_disclosure_deliveries ORDER BY rowid")
            .fetch_all(&self.host.pool)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn posts_once_per_session_in_the_confirmed_call_after_claiming_it() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |_| {}).await;
    let report = run
        .record(capture("s1", "c1"), Authority::with_call(ZOOM))
        .await
        .unwrap();
    assert_eq!(report.outcome, NoticeOutcome::Sent);
    assert_eq!(
        *lock(&run.host.posts),
        vec![NoticeTarget {
            app: NativeMeetingApp::Zoom,
            pid: 11
        }]
    );
    assert_eq!(*lock(&run.host.claimed_at_post), vec![true]);
    // A second recording of the session posts nothing.
    assert!(
        run.record(capture("s1", "c2"), Authority::with_call(ZOOM))
            .await
            .is_none()
    );
    assert_eq!(run.posts(), 1);
    assert_eq!(run.deliveries().await, vec![("sent".into(), "".into())]);
}

#[tokio::test]
async fn posts_nothing_with_the_setting_off_or_signed_out() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |host| {
        host.enabled = AtomicBool::new(false);
    })
    .await;
    assert!(
        run.record(capture("s1", "c1"), Authority::with_call(ZOOM))
            .await
            .is_none()
    );
    run.host.enabled.store(true, Ordering::SeqCst);
    let signed_out = CaptureIdentity {
        account_id: "".into(),
        ..capture("s2", "c2")
    };
    assert!(
        run.record(signed_out, Authority::with_call(ZOOM))
            .await
            .is_none()
    );
    assert_eq!((run.posts(), run.claims().await), (0, 0));
    // Turned on later, the same session still gets its notice.
    assert!(
        run.record(capture("s1", "c3"), Authority::with_call(ZOOM))
            .await
            .is_some()
    );
    assert_eq!(run.posts(), 1);
}

#[tokio::test]
async fn leaves_a_browser_call_and_an_attached_extension_to_the_extension() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |_| {}).await;
    let report = run
        .record(
            capture("s1", "c1"),
            Authority::with_call("com.google.Chrome"),
        )
        .await
        .unwrap();
    assert_eq!(
        (report.outcome, report.reason),
        (NoticeOutcome::NotSent, "extension_required")
    );
    let attached = Authority::with_call(ZOOM);
    attached.extension.store(true, Ordering::SeqCst);
    assert!(run.record(capture("s2", "c2"), attached).await.is_none());
    assert_eq!((run.posts(), run.claims().await), (0, 0));
}

#[tokio::test]
async fn a_recording_without_a_confirmed_call_posts_only_through_the_discovering_sender() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let before = run_with(&path, |host| {
        host.discovered = Mutex::new([Outcome::NoCall, Outcome::ChatUnavailable].into());
    })
    .await;
    // Legacy-engine recordings, and native ones that confirmed no call, never reach the native
    // poster: the discovering sender tries again while it definitely sent nothing, under the claim.
    let report = before
        .record(capture("s1", "c1"), Authority::without_call())
        .await
        .unwrap();
    assert_eq!(report.outcome, NoticeOutcome::Sent);
    assert_eq!(*lock(&before.host.attempts), vec![true, true, true]);
    assert_eq!(before.posts(), 0);
    assert_eq!(before.platforms().await, vec!["slack_huddle".to_string()]);
    // Neither a later recording of the session nor, after a restart, a recording that confirmed
    // its call on the other engine posts it again.
    assert!(
        before
            .record(capture("s1", "c2"), Authority::without_call())
            .await
            .is_none()
    );
    let after = run_with(&path, |_| {}).await;
    assert!(
        after
            .record(capture("s1", "c3"), Authority::with_call(ZOOM))
            .await
            .is_none()
    );
    assert_eq!((before.attempts(), after.posts()), (3, 0));

    // The other way round: a notice posted in the confirmed call holds against the discovering
    // sender after a restart.
    assert!(
        after
            .record(capture("s2", "c4"), Authority::with_call(ZOOM))
            .await
            .is_some()
    );
    let again = run_with(&path, |host| {
        host.discovered = Mutex::new(vec![Outcome::ChatUnavailable; DISCOVERY_ATTEMPTS].into());
    })
    .await;
    assert!(
        again
            .record(capture("s2", "c5"), Authority::without_call())
            .await
            .is_none()
    );
    assert_eq!(again.attempts(), 0);

    // Attempts are bounded: then the notice ends unsent and its claim goes back.
    let report = again
        .record(capture("s3", "c6"), Authority::without_call())
        .await
        .unwrap();
    assert_eq!(
        (report.outcome, report.reason),
        (NoticeOutcome::NotSent, "chat_unavailable")
    );
    assert_eq!(
        (again.attempts(), again.claims().await),
        (DISCOVERY_ATTEMPTS, 2)
    );
}

#[tokio::test]
async fn a_send_press_that_may_have_sent_is_never_retried_even_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let before = run_with(&path, |host| {
        host.discovered = Mutex::new([Outcome::Unconfirmed].into());
    })
    .await;
    let report = before
        .record(capture("s1", "c1"), Authority::without_call())
        .await
        .unwrap();
    assert_eq!(report.outcome, NoticeOutcome::Unconfirmed);
    assert_eq!(before.attempts(), 1);
    assert_eq!(
        before.deliveries().await,
        vec![("unconfirmed".into(), "unconfirmed".into())]
    );
    let after = run_with(&path, |_| {}).await;
    assert!(
        after
            .record(capture("s1", "c2"), Authority::without_call())
            .await
            .is_none()
    );
    assert_eq!(after.attempts(), 0);
}

#[test]
fn a_failed_send_press_reads_as_unconfirmed_not_unsent() {
    let result = |sent: bool, send_action: Option<&str>| anlg_detect::MeetingChatSendResult {
        sent,
        app: Some(anlg_detect::MeetingApp {
            id: "com.tinyspeck.slackmacgap".into(),
            name: "Slack".into(),
        }),
        platform: anlg_detect::MeetingPlatform::Slack,
        surface: anlg_detect::MeetingSurface::Native,
        input_label: None,
        send_action: send_action.map(str::to_string),
        warnings: Vec::new(),
    };
    assert_eq!(
        DiscoveredPost::of(&result(false, Some("sendButton"))).result,
        PostResult::Posted(Outcome::Unconfirmed)
    );
    assert_eq!(
        DiscoveredPost::of(&result(true, Some("sendButton"))).result,
        PostResult::Posted(Outcome::Sent)
    );
    let unsent = DiscoveredPost::of(&result(false, None));
    assert_eq!(unsent.result, PostResult::Posted(Outcome::ChatUnavailable));
    assert_eq!(unsent.platform, "slack_huddle");
}

#[tokio::test]
async fn a_stop_or_an_attaching_extension_ends_the_discovering_sender_before_its_next_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let hold = Arc::new(Notify::new());
    let run = run_with(&dir.path().join("app.db"), |host| {
        host.discovered = Mutex::new([Outcome::ChatUnavailable].into());
        host.hold_retry = Some(hold.clone());
    })
    .await;
    // Signed out (or stopped) while waiting to try again: nothing more is tried, the claim goes
    // back, and a later recording of the session may post.
    let authority = Authority::without_call();
    let work = run
        .dispatcher
        .start(capture("s1", "c1"), authority.clone())
        .unwrap();
    let pending = tokio::spawn(work);
    while run.attempts() == 0 {
        tokio::task::yield_now().await;
    }
    authority.current.store(false, Ordering::SeqCst);
    hold.notify_waiters();
    assert_eq!(
        pending.await.unwrap().unwrap().outcome,
        NoticeOutcome::CancelledBeforeDispatch
    );
    assert_eq!((run.attempts(), run.claims().await), (1, 0));

    // The extension attaches while an attempt waits for the Accessibility thread: it never runs.
    let authority = Authority::without_call();
    let attaching = authority.clone();
    *lock(&run.host.at_post) = Some(Box::new(move || {
        attaching.extension.store(true, Ordering::SeqCst);
    }));
    let report = run.record(capture("s1", "c2"), authority).await.unwrap();
    assert_eq!(report.outcome, NoticeOutcome::CancelledBeforeDispatch);
    assert_eq!((run.attempts(), run.claims().await), (1, 0));
}

#[tokio::test]
async fn a_stop_while_the_call_settles_claims_nothing_and_a_later_recording_posts() {
    let dir = tempfile::tempdir().unwrap();
    let hold = Arc::new(Notify::new());
    let run = run_with(&dir.path().join("app.db"), |host| {
        host.hold_settle = Some(hold.clone());
    })
    .await;
    let first = capture("s1", "c1");
    let work = run
        .dispatcher
        .start(first.clone(), Authority::with_call(ZOOM))
        .unwrap();
    let pending = tokio::spawn(work);
    tokio::task::yield_now().await;
    run.dispatcher.cancel(&first);
    assert!(pending.await.unwrap().is_none());
    assert_eq!((run.posts(), run.claims().await), (0, 0));

    let work = run
        .dispatcher
        .start(capture("s1", "c2"), Authority::with_call(ZOOM))
        .unwrap();
    let pending = tokio::spawn(work);
    tokio::task::yield_now().await;
    hold.notify_waiters();
    assert_eq!(pending.await.unwrap().unwrap().outcome, NoticeOutcome::Sent);
}

#[tokio::test]
async fn a_cancel_before_return_reaches_the_poster_and_gives_the_claim_back() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |host| {
        host.answer = Mutex::new(Answer::OnCancel(Outcome::Cancelled));
    })
    .await;
    let first = capture("s1", "c1");
    let work = run
        .dispatcher
        .start(first.clone(), Authority::with_call(ZOOM))
        .unwrap();
    let pending = tokio::spawn(work);
    while run.posts() == 0 {
        tokio::task::yield_now().await;
    }
    run.dispatcher.cancel(&first);
    assert_eq!(
        pending.await.unwrap().unwrap().outcome,
        NoticeOutcome::CancelledBeforeDispatch
    );
    assert_eq!(run.claims().await, 0);
    // The session never got its notice, so recording it again posts.
    *lock(&run.host.answer) = Answer::Now(Outcome::Sent);
    assert_eq!(
        run.record(capture("s1", "c2"), Authority::with_call(ZOOM))
            .await
            .unwrap()
            .outcome,
        NoticeOutcome::Sent
    );
    assert_eq!(run.posts(), 2);
}

#[tokio::test]
async fn a_stop_after_return_keeps_the_claim_across_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let before = run_with(&path, |host| {
        host.answer = Mutex::new(Answer::OnCancel(Outcome::Unconfirmed));
    })
    .await;
    let first = capture("s1", "c1");
    let work = before
        .dispatcher
        .start(first.clone(), Authority::with_call(ZOOM))
        .unwrap();
    let pending = tokio::spawn(work);
    while before.posts() == 0 {
        tokio::task::yield_now().await;
    }
    before.dispatcher.cancel(&first);
    assert_eq!(
        pending.await.unwrap().unwrap().outcome,
        NoticeOutcome::Unconfirmed
    );
    assert_eq!(
        before.deliveries().await,
        vec![("unconfirmed".into(), "unconfirmed".into())]
    );
    let after = run_with(&path, |_| {}).await;
    assert!(
        after
            .record(capture("s1", "c2"), Authority::with_call(ZOOM))
            .await
            .is_none()
    );
    assert_eq!(after.posts(), 0);
}

#[tokio::test]
async fn a_crash_before_the_app_answered_never_posts_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let before = run_with(&path, |host| {
        host.answer = Mutex::new(Answer::Never);
    })
    .await;
    let work = before
        .dispatcher
        .start(capture("s1", "c1"), Authority::with_call(ZOOM))
        .unwrap();
    let crashed = tokio::spawn(work);
    while before.posts() == 0 {
        tokio::task::yield_now().await;
    }
    crashed.abort();
    let after = run_with(&path, |_| {}).await;
    assert!(
        after
            .record(capture("s1", "c2"), Authority::with_call(ZOOM))
            .await
            .is_none()
    );
    assert_eq!(after.posts(), 0);
}

#[tokio::test]
async fn a_post_that_sent_nothing_gives_the_claim_back_for_a_later_recording() {
    for outcome in [
        Outcome::NoCall,
        Outcome::ChatUnavailable,
        Outcome::DraftInProgress,
        Outcome::NotSent,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let run = run_with(&dir.path().join("app.db"), |host| {
            host.answer = Mutex::new(Answer::Now(outcome));
        })
        .await;
        let report = run
            .record(capture("s1", "c1"), Authority::with_call(ZOOM))
            .await
            .unwrap();
        assert_eq!(
            (report.outcome, report.reason),
            (NoticeOutcome::NotSent, outcome.as_str())
        );
        assert_eq!((run.posts(), run.claims().await), (1, 0));
        // Nothing went out, so recording the session again, in this run or after a restart, posts.
        *lock(&run.host.answer) = Answer::Now(Outcome::Sent);
        assert_eq!(
            run.record(capture("s1", "c2"), Authority::with_call(ZOOM))
                .await
                .unwrap()
                .outcome,
            NoticeOutcome::Sent
        );
        assert_eq!(run.posts(), 2);
    }
}

#[tokio::test]
async fn a_refused_post_releases_and_a_lost_one_holds() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |host| {
        host.answer = Mutex::new(Answer::Refused);
    })
    .await;
    run.record(capture("s1", "c1"), Authority::with_call(ZOOM))
        .await;
    assert_eq!(run.claims().await, 0);
    *lock(&run.host.answer) = Answer::Lost;
    let report = run
        .record(capture("s2", "c2"), Authority::with_call(ZOOM))
        .await
        .unwrap();
    assert_eq!(report.outcome, NoticeOutcome::Unconfirmed);
    assert_eq!(run.claims().await, 1);
}

#[tokio::test]
async fn posts_nothing_when_the_claim_cannot_be_written() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |_| {}).await;
    sqlx::query("DROP TABLE local_meeting_notice_claims")
        .execute(&run.host.pool)
        .await
        .unwrap();
    let report = run
        .record(capture("s1", "c1"), Authority::with_call(ZOOM))
        .await
        .unwrap();
    assert_eq!(
        (report.outcome, report.reason),
        (NoticeOutcome::NotSent, "claim_unavailable")
    );
    assert_eq!(run.posts(), 0);
}

#[tokio::test]
async fn a_capture_that_ends_during_the_claim_write_gives_it_back_unsent() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |_| {}).await;
    let authority = Authority::with_call(ZOOM);
    // The account changes while the claim is written: the capture is no longer current.
    let store = Switching {
        pool: run.host.pool.clone(),
        authority: authority.clone(),
    };
    let dispatcher = Arc::new(NoticeDispatcher::new(run.host.clone(), store));
    let report = dispatcher
        .start(capture("s1", "c1"), authority)
        .unwrap()
        .await
        .unwrap();
    assert_eq!(report.outcome, NoticeOutcome::CancelledBeforeDispatch);
    assert_eq!((run.posts(), run.claims().await), (0, 0));
}

/// A store whose claim write ends with someone else signed in.
struct Switching {
    pool: sqlx::SqlitePool,
    authority: Arc<Authority>,
}

impl NoticeStore for Switching {
    async fn claim(&self, capture: &CaptureIdentity, now_ms: i64) -> Result<bool, String> {
        let claimed = self.pool.claim(capture, now_ms).await;
        self.authority.current.store(false, Ordering::SeqCst);
        claimed
    }

    async fn settle(
        &self,
        capture: &CaptureIdentity,
        settlement: Settlement,
        now_ms: i64,
    ) -> Result<bool, String> {
        self.pool.settle(capture, settlement, now_ms).await
    }

    async fn release(&self, capture: &CaptureIdentity) -> Result<bool, String> {
        self.pool.release(capture).await
    }

    async fn record(&self, capture: &CaptureIdentity, delivery: Delivery) -> Result<(), String> {
        self.pool.record(capture, delivery).await
    }
}

#[tokio::test]
async fn two_recordings_of_one_session_starting_together_post_once() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |_| {}).await;
    let first = run
        .dispatcher
        .start(capture("s1", "c1"), Authority::with_call(ZOOM))
        .unwrap();
    // The same capture announced twice runs one notice.
    assert!(
        run.dispatcher
            .start(capture("s1", "c1"), Authority::with_call(ZOOM))
            .is_none()
    );
    let second = run
        .dispatcher
        .start(capture("s1", "c2"), Authority::with_call(ZOOM))
        .unwrap();
    // Two app runs over the same database (two windows racing) start one more.
    let other = Arc::new(NoticeDispatcher::new(
        run.host.clone(),
        run.host.pool.clone(),
    ));
    let third = other
        .start(capture("s1", "c3"), Authority::without_call())
        .unwrap();
    let reports = tokio::join!(first, second, third);
    let sent = [reports.0, reports.1, reports.2]
        .into_iter()
        .flatten()
        .filter(|report| report.outcome == NoticeOutcome::Sent)
        .count();
    assert_eq!((sent, run.posts() + run.attempts()), (1, 1));
}

#[tokio::test]
async fn an_extension_attaching_during_the_post_stops_it_before_return() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |host| {
        host.answer = Mutex::new(Answer::OnCancel(Outcome::Cancelled));
    })
    .await;
    let first = capture("s1", "c1");
    let dispatcher = run.dispatcher.clone();
    let cancel_target = first.clone();
    // The observer cancels the native run when the extension attaches mid-post.
    *lock(&run.host.at_post) = Some(Box::new(move || dispatcher.cancel(&cancel_target)));
    let report = run
        .dispatcher
        .start(first, Authority::with_call(ZOOM))
        .unwrap()
        .await
        .unwrap();
    assert_eq!(report.outcome, NoticeOutcome::CancelledBeforeDispatch);
    assert_eq!(run.claims().await, 0);
}

#[tokio::test]
async fn the_extension_and_native_dispatch_share_one_claim() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |_| {}).await;
    let attached = capture("s1", "c1");
    let authority = Authority::with_call("com.google.Chrome");
    let native = run
        .dispatcher
        .start(attached.clone(), authority.clone())
        .unwrap();
    let text = run
        .dispatcher
        .claim_for_extension(&attached, authority.as_ref())
        .await;
    assert_eq!(text.as_deref(), Some(run.host.message().as_str()));
    // A cancelled native task may finish after the extension has received its notice; its
    // cleanup leaves the extension's claim alone.
    run.dispatcher.cancel(&attached);
    assert!(native.await.is_none());
    // Handed out once: a second ask, or a native capture of the session, gets nothing.
    assert!(
        run.dispatcher
            .claim_for_extension(&attached, authority.as_ref())
            .await
            .is_none()
    );
    run.dispatcher
        .settle_for_extension(&attached, NoticeOutcome::Unconfirmed, "unconfirmed")
        .await;
    run.dispatcher.cancel(&attached);
    // A recording with no confirmed call, here or after a restart, never sends it again.
    assert!(
        run.record(capture("s1", "c-legacy"), Authority::without_call())
            .await
            .is_none()
    );
    assert_eq!(run.attempts(), 0);
    let restarted = Arc::new(NoticeDispatcher::new(
        run.host.clone(),
        run.host.pool.clone(),
    ));
    assert!(
        restarted
            .start(capture("s1", "c2"), Authority::with_call(ZOOM))
            .unwrap()
            .await
            .is_none()
    );
    assert_eq!(run.posts(), 0);
}

#[tokio::test]
async fn an_extension_notice_that_sent_nothing_gives_the_claim_back() {
    let dir = tempfile::tempdir().unwrap();
    let run = run_with(&dir.path().join("app.db"), |_| {}).await;
    let attached = capture("s1", "c1");
    let authority = Authority::with_call("com.google.Chrome");
    // Attached to a native recording of a Meet call in Chrome, which native dispatch leaves alone.
    let native = run
        .dispatcher
        .start(attached.clone(), authority.clone())
        .unwrap();
    assert_eq!(native.await.unwrap().reason, "extension_required");
    assert!(
        run.dispatcher
            .claim_for_extension(&attached, authority.as_ref())
            .await
            .is_some()
    );
    run.dispatcher
        .settle_for_extension(&attached, NoticeOutcome::NotSent, "chat_unavailable")
        .await;
    assert_eq!(run.claims().await, 0);
    assert_eq!(
        run.deliveries().await,
        vec![
            ("not_sent".into(), "extension_required".into()),
            ("not_sent".into(), "chat_unavailable".into())
        ]
    );
    // Not current, or no storage: nothing is handed out.
    authority.current.store(false, Ordering::SeqCst);
    assert!(
        run.dispatcher
            .claim_for_extension(&attached, authority.as_ref())
            .await
            .is_none()
    );
    authority.current.store(true, Ordering::SeqCst);
    sqlx::query("DROP TABLE local_meeting_notice_claims")
        .execute(&run.host.pool)
        .await
        .unwrap();
    assert!(
        run.dispatcher
            .claim_for_extension(&attached, authority.as_ref())
            .await
            .is_none()
    );
}
