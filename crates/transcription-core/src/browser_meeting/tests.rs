use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio::sync::Notify;

use super::*;

const MEET: &str = "https://meet.google.com/abc-defg-hij";
const OTHER_MEET: &str = "https://meet.google.com/xxx-yyyy-zzz";
const CHROME_PID: u32 = 77;
const ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Cap {
    account: String,
    id: String,
}

struct FakeCapture {
    session_id: String,
    owner: Owner,
    status: RecorderStatus,
    pausable: bool,
    confirmed: Option<RecordedCall>,
}

#[derive(Default)]
struct World {
    account: Option<String>,
    clock: u64,
    next: u32,
    captures: HashMap<Cap, FakeCapture>,
    links: HashMap<String, Cap>,
    bundles: HashMap<u32, String>,
    /// What the recorder confirms for an extension start; `None` as the legacy engine does.
    start_confirms: Option<RecordedCall>,
    calls: Vec<String>,
    observations: Vec<MeetObservation>,
    notice: Option<String>,
    camera: Option<String>,
    claimed: HashSet<(Cap, &'static str)>,
    outcomes: Vec<String>,
}

/// The meeting runtime and recorder as integration wires them, over one in-memory world.
#[derive(Clone, Default)]
struct Fake {
    world: Arc<Mutex<World>>,
    /// Holds an extension start until notified, as a slow permission prompt does.
    start_gate: Option<Arc<Notify>>,
    /// Holds a notice claim until notified.
    claim_gate: Option<Arc<Notify>>,
    /// Holds the recorder's pause acknowledgement until notified.
    pause_gate: Option<Arc<Notify>>,
}

/// A start dropped before it resolved: the capture it began stops, as the port promises.
struct CancelStart {
    fake: Fake,
    cap: Option<Cap>,
}

impl Drop for CancelStart {
    fn drop(&mut self) {
        if let Some(cap) = self.cap.take() {
            let mut world = self.fake.world();
            world.calls.push(format!("cancel:{}", cap.id));
            world.captures.remove(&cap);
            world.links.retain(|_, linked| *linked != cap);
        }
    }
}

impl Fake {
    fn new() -> Self {
        let fake = Self::default();
        {
            let mut world = fake.world();
            world.account = Some("account-1".into());
            world.bundles.insert(CHROME_PID, "com.google.Chrome".into());
            world.start_confirms = Some(chrome(Some(MEET)));
            world.notice = Some("Anarlog is taking notes.".into());
            world.camera = Some("Anarlog Watermark".into());
        }
        fake
    }

    fn world(&self) -> std::sync::MutexGuard<'_, World> {
        self.world.lock().unwrap()
    }

    /// A recording Anarlog's own window started, confirmed on `confirmed`.
    fn app_recording(&self, status: RecorderStatus, confirmed: Option<RecordedCall>) -> Cap {
        let mut world = self.world();
        world.next += 1;
        let cap = Cap {
            account: world.account.clone().unwrap(),
            id: format!("capture-{}", world.next),
        };
        world.captures.insert(
            cap.clone(),
            FakeCapture {
                session_id: "from-app".into(),
                owner: Owner::App,
                status,
                pausable: true,
                confirmed,
            },
        );
        cap
    }

    fn set_status(&self, cap: &Cap, status: RecorderStatus) {
        self.world().captures.get_mut(cap).unwrap().status = status;
    }

    fn calls(&self) -> Vec<String> {
        self.world().calls.clone()
    }

    fn wait(&self, ms: u64) {
        self.world().clock += ms;
    }
}

fn chrome(url: Option<&str>) -> RecordedCall {
    RecordedCall {
        bundle_id: "com.google.Chrome".into(),
        call_url: url.map(str::to_owned),
    }
}

impl BrowserMeetingPort for Fake {
    type Capture = Cap;

    fn account(&self) -> Option<String> {
        self.world().account.clone()
    }

    fn now_ms(&self) -> u64 {
        self.world().clock
    }

    fn active(&self) -> Option<RecorderView<Cap>> {
        let cap = self.world().captures.keys().next().cloned()?;
        self.capture(&cap)
    }

    fn capture(&self, cap: &Cap) -> Option<RecorderView<Cap>> {
        let world = self.world();
        if world.account.as_deref() != Some(cap.account.as_str()) {
            return None;
        }
        world.captures.get(cap).map(|capture| RecorderView {
            capture: cap.clone(),
            session_id: capture.session_id.clone(),
            owner: capture.owner,
            status: capture.status.clone(),
            pausable: capture.pausable,
        })
    }

    fn recorded_call(&self, cap: &Cap) -> Option<RecordedCall> {
        self.world().captures.get(cap)?.confirmed.clone()
    }

    fn bundle_id_of(&self, pid: u32) -> Option<String> {
        self.world().bundles.get(&pid).cloned()
    }

    fn link(&self, cap: &Cap, call_url: &str) -> Result<String, PortError> {
        let mut world = self.world();
        let confirmed = world
            .captures
            .get(cap)
            .ok_or(PortError::Stale)?
            .confirmed
            .clone();
        let confirmed_url = confirmed
            .and_then(|call| call.call_url)
            .ok_or(PortError::CallMismatch)?;
        if !same_meet_call(&confirmed_url, call_url) {
            return Err(PortError::CallMismatch);
        }
        world.links.retain(|_, linked| linked != cap);
        world.next += 1;
        let token = format!("link-{}", world.next);
        world.links.insert(token.clone(), cap.clone());
        Ok(token)
    }

    fn resolve_link(&self, token: &str) -> Result<Cap, PortError> {
        let world = self.world();
        let cap = world.links.get(token).ok_or(PortError::Stale)?;
        world
            .captures
            .contains_key(cap)
            .then(|| cap.clone())
            .ok_or(PortError::Stale)
    }

    fn detach(&self, token: &str) -> Result<DetachOutcome, PortError> {
        let mut world = self.world();
        let cap = world.links.remove(token).ok_or(PortError::Stale)?;
        let owner = world.captures.get(&cap).ok_or(PortError::Stale)?.owner;
        world.calls.push(format!("detach:{}", cap.id));
        Ok(match owner {
            Owner::App => DetachOutcome::KeepRecording,
            Owner::Extension => DetachOutcome::StopRecording,
        })
    }

    async fn start(&self, request: ExtensionStart) -> Result<Cap, PortError> {
        let cap = {
            let mut world = self.world();
            world.calls.push(format!("start:{}", request.call_url));
            world.next += 1;
            let next = world.next;
            let cap = Cap {
                account: request.account_id.clone(),
                id: format!("capture-{next}"),
            };
            world.captures.insert(
                cap.clone(),
                FakeCapture {
                    session_id: format!("session-{next}"),
                    owner: Owner::Extension,
                    status: RecorderStatus::Starting,
                    pausable: true,
                    confirmed: None,
                },
            );
            cap
        };
        let mut cancel = CancelStart {
            fake: self.clone(),
            cap: Some(cap.clone()),
        };
        if let Some(gate) = &self.start_gate {
            gate.notified().await;
        }
        cancel.cap = None;
        let mut world = self.world();
        let confirms = world.start_confirms.clone();
        if let Some(capture) = world.captures.get_mut(&cap) {
            capture.status = RecorderStatus::Recording;
            capture.confirmed = confirms;
        }
        Ok(cap)
    }

    async fn pause(&self, cap: &Cap) -> Result<(), PortError> {
        self.world().calls.push(format!("pause:{}", cap.id));
        if let Some(gate) = &self.pause_gate {
            gate.notified().await;
        }
        self.set_status(cap, RecorderStatus::Paused);
        Ok(())
    }

    async fn resume(&self, cap: &Cap) -> Result<(), PortError> {
        self.world().calls.push(format!("resume:{}", cap.id));
        self.set_status(cap, RecorderStatus::Recording);
        Ok(())
    }

    async fn stop(&self, cap: &Cap) -> Result<(), PortError> {
        let mut world = self.world();
        world.calls.push(format!("stop:{}", cap.id));
        world.captures.remove(cap);
        world.links.retain(|_, linked| linked != cap);
        Ok(())
    }

    fn observe(&self, _cap: &Cap, observation: MeetObservation) {
        self.world().observations.push(observation);
    }

    async fn claim_notice(&self, cap: &Cap) -> Option<String> {
        if let Some(gate) = &self.claim_gate {
            gate.notified().await;
        }
        let mut world = self.world();
        let notice = world.notice.clone()?;
        world
            .claimed
            .insert((cap.clone(), "notice"))
            .then_some(notice)
    }

    fn notice_outcome(&self, cap: &Cap, outcome: ChatOutcome) {
        let mut world = self.world();
        world.outcomes.push(format!("chat:{outcome:?}"));
        if !outcome.may_have_sent() {
            world.claimed.remove(&(cap.clone(), "notice"));
        }
    }

    async fn claim_camera(&self, cap: &Cap) -> Option<String> {
        let mut world = self.world();
        let camera = world.camera.clone()?;
        world
            .claimed
            .insert((cap.clone(), "camera"))
            .then_some(camera)
    }

    fn camera_outcome(&self, cap: &Cap, outcome: CameraOutcome) {
        let mut world = self.world();
        world.outcomes.push(format!("camera:{outcome:?}"));
        if outcome.untouched() {
            world.claimed.remove(&(cap.clone(), "camera"));
        }
    }
}

fn peer() -> Peer {
    Peer {
        origin: ORIGIN.into(),
        browser_pid: Some(CHROME_PID),
    }
}

fn new_route(fake: &Fake) -> BrowserMeetingRoute<Fake> {
    BrowserMeetingRoute::new(fake.clone())
}

fn code(result: Result<Linked, Refusal>) -> RefusalCode {
    result.expect_err("refused").code
}

#[tokio::test]
async fn an_extension_start_goes_through_the_one_recorder_and_links_only_once_confirmed() {
    let fake = Fake::new();
    let route = new_route(&fake);
    for url in [
        "https://meet.google.com/home",
        "https://example.com/abc-defg-hij",
        "https://meet.google.com/abc-defg-hij?authuser=1",
    ] {
        assert_eq!(
            code(route.start(url, &peer()).await),
            RefusalCode::BadRequest
        );
    }
    let mut relay = Relay::new(peer());
    let events = relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    assert_eq!(
        events,
        vec![ExtensionEvent::Started {
            pausable: true,
            notice: Some("Anarlog is taking notes.".into()),
            camera: Some("Anarlog Watermark".into()),
        }]
    );
    assert_eq!(fake.calls(), vec![format!("start:{MEET}")]);

    // A legacy-engine or System-scope recording never confirms the Meet call: the tab's own start
    // stops again rather than linking cues to audio of something else.
    let fake = Fake::new();
    fake.world().start_confirms = None;
    let route = new_route(&fake);
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::CallUnconfirmed
    );
    fake.world().start_confirms = Some(chrome(Some(OTHER_MEET)));
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::CallUnconfirmed
    );
    assert_eq!(
        fake.calls(),
        vec![
            format!("start:{MEET}"),
            "stop:capture-1".into(),
            format!("start:{MEET}"),
            "stop:capture-2".into(),
        ]
    );
    assert!(fake.world().captures.is_empty());

    fake.world().account = None;
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::AppNotReady
    );
}

#[tokio::test]
async fn extension_start_stops_instead_of_linking_the_same_call_in_another_browser() {
    let fake = Fake::new();
    fake.world().start_confirms = Some(RecordedCall {
        bundle_id: "com.microsoft.edgemac".into(),
        call_url: Some(MEET.into()),
    });
    let route = new_route(&fake);

    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::CallUnconfirmed
    );
    assert!(
        fake.active().is_none(),
        "the mismatched recording must stop"
    );
    assert!(fake.world().links.is_empty());
    assert!(fake.world().claimed.is_empty());
}

#[tokio::test]
async fn links_only_the_confirmed_meet_call_in_the_same_browser() {
    let fake = Fake::new();
    let route = new_route(&fake);
    let cap = fake.app_recording(RecorderStatus::Starting, Some(chrome(Some(MEET))));
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::CallUnconfirmed
    );
    fake.set_status(&cap, RecorderStatus::Recording);
    let set_call = |call: Option<RecordedCall>| {
        fake.world().captures.get_mut(&cap).unwrap().confirmed = call;
    };
    for (call, url, expected) in [
        (
            Some(RecordedCall {
                bundle_id: "us.zoom.xos".into(),
                call_url: None,
            }),
            MEET,
            RefusalCode::OtherCall,
        ),
        // A whole-system recording, or an old helper that confirmed no call.
        (None, MEET, RefusalCode::CallUnconfirmed),
        // An old helper that names the browser but not the call's address.
        (Some(chrome(None)), MEET, RefusalCode::CallUnconfirmed),
        (
            Some(chrome(Some("https://meet.google.com/"))),
            MEET,
            RefusalCode::CallUnconfirmed,
        ),
        (
            Some(RecordedCall {
                bundle_id: "com.microsoft.edgemac".into(),
                call_url: Some(MEET.into()),
            }),
            MEET,
            RefusalCode::OtherCall,
        ),
        (Some(chrome(Some(MEET))), OTHER_MEET, RefusalCode::OtherCall),
    ] {
        set_call(call);
        assert_eq!(code(route.start(url, &peer()).await), expected);
    }
    set_call(Some(RecordedCall {
        bundle_id: "com.google.Chrome.helper".into(),
        call_url: Some(format!("{MEET}?authuser=1#x")),
    }));
    // Another Chrome profile is the same browser; another Chrome build is not.
    fake.world()
        .bundles
        .insert(CHROME_PID, "com.google.Chrome.canary".into());
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::OtherCall
    );
    fake.world().bundles.remove(&CHROME_PID);
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::CallUnconfirmed
    );
    let unnamed = Peer {
        browser_pid: None,
        ..peer()
    };
    assert_eq!(
        code(route.start(MEET, &unnamed).await),
        RefusalCode::CallUnconfirmed
    );
    assert!(fake.world().links.is_empty());

    fake.world()
        .bundles
        .insert(CHROME_PID, "com.google.Chrome".into());
    let linked = route.start(MEET, &peer()).await.unwrap();
    assert!(linked.attached);
    assert_eq!(linked.session_id, "from-app");
    assert!(fake.calls().is_empty(), "no second recording starts");
}

#[tokio::test]
async fn wrong_stale_or_other_account_links_cannot_act_or_add_cues() {
    let fake = Fake::new();
    let route = new_route(&fake);
    let first = route.start(MEET, &peer()).await.unwrap();
    let events = json!([{"kind": "speaker", "at": 1, "participantId": "p-alice", "name": "Alice"}]);
    let events = events.as_array().unwrap();

    assert_eq!(
        route.poll("guessed").unwrap_err().code,
        RefusalCode::NotLinked
    );
    assert!(route.cues("guessed", events).is_err());
    assert_eq!(
        route
            .command("guessed", Command::Stop)
            .await
            .unwrap_err()
            .code,
        RefusalCode::NotLinked
    );
    route.cues(&first.link, events).unwrap();
    assert_eq!(fake.world().observations.len(), 1);

    // The runtime replaced the token behind the route's back: the runtime is the authority.
    let cap = fake.world().links.remove(&first.link).unwrap();
    fake.world().links.insert("elsewhere".into(), cap);
    assert_eq!(
        route.poll(&first.link).unwrap_err().code,
        RefusalCode::NotLinked
    );

    // Another account signed in: the old account's capture is no longer this tab's to act on.
    let fake = Fake::new();
    let route = new_route(&fake);
    let linked = route.start(MEET, &peer()).await.unwrap();
    fake.world().account = Some("account-2".into());
    assert_eq!(route.poll(&linked.link), Ok(Status::Ended));
    assert!(route.cues(&linked.link, events).is_err());
    assert!(!fake.calls().iter().any(|call| call.starts_with("stop")));
}

#[tokio::test]
async fn takes_over_a_stale_link_on_the_same_call_only_and_keeps_the_owner() {
    let fake = Fake::new();
    let route = new_route(&fake);
    let first = route.start(MEET, &peer()).await.unwrap();
    assert!(!first.attached);

    fake.wait(10_000);
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::LinkedElsewhere
    );
    // The first tab still asks, so its hold stays fresh.
    route.poll(&first.link).unwrap();
    fake.wait(10_000);
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::LinkedElsewhere
    );
    // Then its native host dies without letting go.
    fake.wait(16_000);
    assert_eq!(
        code(route.start(OTHER_MEET, &peer()).await),
        RefusalCode::OtherCall
    );
    let second = route.start(MEET, &peer()).await.unwrap();
    // Still the extension's recording: its call ending stops it.
    assert!(!second.attached);
    assert_eq!(second.session_id, first.session_id);
    assert_eq!(
        route
            .command(&first.link, Command::Pause)
            .await
            .unwrap_err()
            .code,
        RefusalCode::NotLinked
    );
    assert_eq!(fake.calls().len(), 1, "no second start");
}

#[tokio::test]
async fn two_clicks_racing_link_one_tab_including_during_an_extension_start() {
    let gate = Arc::new(Notify::new());
    let fake = Fake {
        start_gate: Some(gate.clone()),
        ..Fake::new()
    };
    let route = Arc::new(new_route(&fake));
    let first = tokio::spawn({
        let route = route.clone();
        async move { route.start(MEET, &peer()).await }
    });
    tokio::task::yield_now().await;
    while fake.calls().is_empty() {
        tokio::task::yield_now().await;
    }
    // The recorder shows a starting capture now, but the tab's own start holds it.
    assert_eq!(
        code(route.start(MEET, &peer()).await),
        RefusalCode::LinkedElsewhere
    );
    gate.notify_one();
    assert!(first.await.unwrap().is_ok());

    let fake = Fake::new();
    fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let route = new_route(&fake);
    let tab = peer();
    let (a, b) = tokio::join!(route.start(MEET, &tab), route.start(MEET, &tab));
    assert_eq!(
        [a.is_ok(), b.is_ok()].iter().filter(|ok| **ok).count(),
        1,
        "{a:?} {b:?}"
    );
}

#[tokio::test]
async fn letting_go_keeps_an_app_recording_and_stops_the_tabs_own() {
    let fake = Fake::new();
    let cap = fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let route = new_route(&fake);
    let mut relay = Relay::new(peer());
    relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    assert_eq!(
        relay.handle(&route, ExtensionMessage::MeetingDetach).await,
        vec![ExtensionEvent::Detached]
    );
    assert!(fake.capture(&cap).is_some());
    // The same tab, clicked again, follows anew; the connection closing lets go the same way.
    let mut relay = Relay::new(peer());
    relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    relay.close(&route).await;
    assert!(fake.capture(&cap).is_some());
    // Only the person's own Stop stops a recording the tab joined.
    let mut relay = Relay::new(peer());
    relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    assert_eq!(
        relay.handle(&route, ExtensionMessage::MeetingStop).await,
        vec![ExtensionEvent::Ended]
    );
    assert!(fake.capture(&cap).is_none());

    let fake = Fake::new();
    let route = new_route(&fake);
    let mut relay = Relay::new(peer());
    relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    // A start that lands after its tab closed: the queued detach stops the tab's own recording.
    assert_eq!(
        relay.handle(&route, ExtensionMessage::MeetingDetach).await,
        vec![ExtensionEvent::Ended]
    );
    assert_eq!(
        fake.calls(),
        vec![
            format!("start:{MEET}"),
            "detach:capture-1".into(),
            "stop:capture-1".into()
        ]
    );
}

#[tokio::test]
async fn pause_resume_and_stop_reach_both_sides_and_unsupported_pause_keeps_the_link() {
    let fake = Fake::new();
    let cap = fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let route = new_route(&fake);
    let mut relay = Relay::new(peer());
    relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    assert_eq!(
        relay.handle(&route, ExtensionMessage::MeetingPause).await,
        vec![ExtensionEvent::Paused]
    );
    assert_eq!(
        relay.handle(&route, ExtensionMessage::MeetingResume).await,
        vec![ExtensionEvent::Resumed]
    );
    // A pause made in Anarlog's own window reaches the tab on the next poll, once.
    fake.set_status(&cap, RecorderStatus::Paused);
    assert_eq!(relay.poll(&route), vec![ExtensionEvent::Paused]);
    assert_eq!(relay.poll(&route), vec![]);
    fake.set_status(&cap, RecorderStatus::Recording);
    assert_eq!(relay.poll(&route), vec![ExtensionEvent::Resumed]);

    fake.world().captures.get_mut(&cap).unwrap().pausable = false;
    assert!(matches!(
        relay.handle(&route, ExtensionMessage::MeetingPause).await[..],
        [ExtensionEvent::Refused { .. }]
    ));
    assert!(relay.linked());
    // Stopped from Anarlog's window: the tab hears it ended.
    fake.world().captures.remove(&cap);
    assert_eq!(relay.poll(&route), vec![ExtensionEvent::Ended]);
    assert!(!relay.linked());
    assert_eq!(
        relay.handle(&route, ExtensionMessage::MeetingStop).await,
        vec![ExtensionEvent::Error {
            message: "Anarlog isn't recording this meeting.".into()
        }]
    );
}

#[tokio::test]
async fn an_app_restart_refuses_old_links_and_says_a_finished_recording_ended() {
    let fake = Fake::new();
    let cap = fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let before = new_route(&fake);
    let linked = before.start(MEET, &peer()).await.unwrap();
    // A new app process: a fresh route and runtime that never handed this token out.
    fake.world().links.clear();
    let after = new_route(&fake);
    assert_eq!(
        after.poll(&linked.link).unwrap_err().code,
        RefusalCode::NotLinked
    );
    // The old route's own hold sees the runtime no longer knows the token.
    assert_eq!(
        before.poll(&linked.link).unwrap_err().code,
        RefusalCode::NotLinked
    );
    let relinked = after.start(MEET, &peer()).await.unwrap();
    fake.world().captures.remove(&cap);
    assert_eq!(after.poll(&relinked.link), Ok(Status::Ended));
}

#[tokio::test]
async fn hands_the_notice_and_camera_once_and_never_again_once_one_may_have_gone_out() {
    let fake = Fake::new();
    fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let route = new_route(&fake);
    let first = route.start(MEET, &peer()).await.unwrap();
    assert_eq!(first.notice.as_deref(), Some("Anarlog is taking notes."));
    assert_eq!(first.camera.as_deref(), Some("Anarlog Watermark"));
    assert_eq!(
        route
            .chat(&first.link, "everyone read it")
            .unwrap_err()
            .code,
        RefusalCode::BadRequest
    );
    assert!(route.chat("guessed", "sent").is_err());
    route.detach(&first.link).await.unwrap();

    // No outcome yet: the notice may still be going out, so a second tab doesn't get it.
    let second = route.start(MEET, &peer()).await.unwrap();
    assert_eq!(
        (second.notice.as_deref(), second.camera.as_deref()),
        (None, None)
    );
    assert_eq!(
        route.chat(&second.link, "sent").unwrap_err().code,
        RefusalCode::NotLinked
    );
    // A late outcome from the first tab, after it let go, isn't taken either.
    assert!(route.chat(&first.link, "not_in_call").is_err());
    assert!(fake.world().outcomes.is_empty());
    route.detach(&second.link).await.unwrap();

    // Nothing sent: the next tab may post it; an unconfirmed send is never posted again.
    let fake = Fake::new();
    let cap = fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let route = new_route(&fake);
    let first = route.start(MEET, &peer()).await.unwrap();
    route.chat(&first.link, "not_in_call").unwrap();
    assert!(
        route.chat(&first.link, "sent").is_err(),
        "one outcome per hand-out"
    );
    route.camera(&first.link, "camera_missing").unwrap();
    route.detach(&first.link).await.unwrap();
    let second = route.start(MEET, &peer()).await.unwrap();
    assert!(second.notice.is_some());
    assert!(
        second.camera.is_none(),
        "a camera attempt that touched Meet isn't retried"
    );
    route.chat(&second.link, "unconfirmed").unwrap();
    route.detach(&second.link).await.unwrap();
    // Even with a claim store that forgot, the route never hands a possibly-sent notice out again.
    fake.world().claimed.clear();
    assert!(route.start(MEET, &peer()).await.unwrap().notice.is_none());
    assert_eq!(
        fake.world().outcomes,
        vec!["chat:NotInCall", "camera:CameraMissing", "chat:Unconfirmed"]
    );

    // A tab that links while the recording is paused is handed nothing.
    fake.world().links.clear();
    fake.wait(LINK_STALE_MS);
    fake.world().claimed.clear();
    fake.set_status(&cap, RecorderStatus::Paused);
    let paused = new_route(&fake);
    let linked = paused.start(MEET, &peer()).await.unwrap();
    assert_eq!(linked.status, Status::Paused);
    assert_eq!((linked.notice, linked.camera), (None, None));
}

#[tokio::test]
async fn a_claim_whose_tab_lost_the_link_meanwhile_is_released_unsent() {
    let gate = Arc::new(Notify::new());
    let fake = Fake {
        claim_gate: Some(gate.clone()),
        ..Fake::new()
    };
    let cap = fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let route = Arc::new(new_route(&fake));
    let start = tokio::spawn({
        let route = route.clone();
        async move { route.start(MEET, &peer()).await }
    });
    while fake.world().links.is_empty() {
        tokio::task::yield_now().await;
    }
    // The recording stops while the claim is still being written.
    fake.world().captures.remove(&cap);
    gate.notify_one();
    assert!(start.await.unwrap().is_err());
    assert_eq!(
        fake.world().outcomes,
        vec!["chat:Cancelled", "camera:Cancelled"]
    );
    assert!(fake.world().claimed.is_empty());
}

#[tokio::test]
async fn cues_reach_the_runtime_only_from_the_linked_tab_and_its_call() {
    let fake = Fake::new();
    let route = new_route(&fake);
    let mut relay = Relay::new(peer());
    let tick = json!([
        {"kind": "speaker", "at": 5, "participantId": "p2", "name": "Alice"},
        {"kind": "roster", "at": 5, "selfId": "p1", "count": 2, "participants": [
            {"participantId": "p1", "name": "John"}, {"participantId": "p2", "name": "Alice"}]},
        {"kind": "heartbeat", "at": 5, "meetingCode": "abc-defg-hij"},
    ]);
    let cues = || ExtensionMessage::MeetingCues {
        events: tick.as_array().unwrap().clone(),
    };
    relay.handle(&route, cues()).await;
    assert!(
        fake.world().observations.is_empty(),
        "nothing before a link"
    );
    relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    relay.handle(&route, cues()).await;
    let moved = ExtensionMessage::MeetingCues {
        events: vec![json!({"kind": "heartbeat", "at": 6, "meetingCode": "xxx-yyyy-zzz"})],
    };
    relay.handle(&route, moved).await;
    relay.handle(&route, ExtensionMessage::MeetingStop).await;
    relay.handle(&route, cues()).await;
    let observations = fake.world().observations.clone();
    assert_eq!(observations.len(), 2);
    assert_eq!(observations[0].speaking, None);
    assert_eq!(
        observations[1]
            .speaking
            .as_deref()
            .map(|cues| cues[0].participant_id.as_str()),
        Some("p2")
    );
}

#[tokio::test]
async fn the_socket_admits_only_this_launch_and_lets_go_when_the_host_goes() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let fake = Fake::new();
    let route = Arc::new(new_route(&fake));
    let admission = Admission {
        secret: "launch-secret".into(),
        allowed_origins: vec![ORIGIN.into()],
    };
    async fn frame(stream: &mut tokio::io::DuplexStream) -> serde_json::Value {
        let mut len = [0u8; 4];
        stream.read_exact(&mut len).await.unwrap();
        let mut body = vec![0u8; u32::from_le_bytes(len) as usize];
        stream.read_exact(&mut body).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }
    let hello = |secret: &str, origin: &str| Hello {
        protocol: PROTOCOL_VERSION,
        secret: secret.into(),
        origin: origin.into(),
        browser_pid: Some(CHROME_PID),
    };

    let (_closing, closing) = tokio::sync::watch::channel(false);
    for refused in [
        hello("guessed", ORIGIN),
        hello("launch-secret", "chrome-extension://someoneelse/"),
        Hello {
            protocol: 0,
            ..hello("launch-secret", ORIGIN)
        },
    ] {
        let (mut host, app) = tokio::io::duplex(4096);
        let serving = tokio::spawn({
            let route = route.clone();
            let admission = admission.clone();
            let closing = closing.clone();
            async move { serve_connection(app, route, &admission, closing).await }
        });
        host.write_all(&encode_frame(&refused).unwrap())
            .await
            .unwrap();
        assert_eq!(frame(&mut host).await["type"], "refused");
        serving.await.unwrap().unwrap();
    }

    let (mut host, app) = tokio::io::duplex(4096);
    let serving = tokio::spawn({
        let route = route.clone();
        let admission = admission.clone();
        let closing = closing.clone();
        async move { serve_connection(app, route, &admission, closing).await }
    });
    host.write_all(&encode_frame(&hello("launch-secret", ORIGIN)).unwrap())
        .await
        .unwrap();
    assert_eq!(frame(&mut host).await["type"], "welcome");
    host.write_all(&encode_frame(&json!({"type": "meeting_start", "url": MEET})).unwrap())
        .await
        .unwrap();
    assert_eq!(frame(&mut host).await["event"], "started");
    // Chrome closed the port: the tab's own recording stops.
    drop(host);
    serving.await.unwrap().unwrap();
    assert!(fake.world().captures.is_empty());
    assert_eq!(
        fake.calls().last().map(String::as_str),
        Some("stop:capture-1")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn serve_writes_a_private_record_and_socket_and_removes_them_on_drop() {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::AsyncWriteExt;

    let dir = tempfile::tempdir().unwrap();
    let endpoints = dir.path().join(ENDPOINT_DIR_NAME);
    let fake = Fake::new();
    let endpoint = serve(
        &endpoints,
        "com.hyprnote.dev",
        vec![ORIGIN.into()],
        Arc::new(new_route(&fake)),
    )
    .unwrap();
    let record_path = endpoints.join("com.hyprnote.dev.json");
    let record: EndpointRecord =
        serde_json::from_slice(&std::fs::read(&record_path).unwrap()).unwrap();
    let mode =
        |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&endpoints), 0o700);
    assert_eq!(mode(&record_path), 0o600);
    assert_eq!(mode(std::path::Path::new(&record.socket)), 0o600);
    assert_eq!(record.allowed_origins, vec![ORIGIN.to_string()]);
    assert!(record.secret.len() >= 64);

    let mut stream = tokio::net::UnixStream::connect(&record.socket)
        .await
        .unwrap();
    let hello = Hello {
        protocol: PROTOCOL_VERSION,
        secret: record.secret.clone(),
        origin: ORIGIN.into(),
        browser_pid: Some(CHROME_PID),
    };
    stream
        .write_all(&encode_frame(&hello).unwrap())
        .await
        .unwrap();
    let mut len = [0u8; 4];
    tokio::io::AsyncReadExt::read_exact(&mut stream, &mut len)
        .await
        .unwrap();

    let socket = record.socket.clone();
    drop(endpoint);
    assert!(!record_path.exists());
    assert!(!std::path::Path::new(&socket).exists());
}

#[tokio::test]
async fn a_tab_that_lets_go_of_a_recording_it_paused_leaves_it_running() {
    let fake = Fake::new();
    let cap = fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let route = new_route(&fake);
    let mut relay = Relay::new(peer());
    relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    relay.handle(&route, ExtensionMessage::MeetingPause).await;
    assert_eq!(
        relay.handle(&route, ExtensionMessage::MeetingDetach).await,
        vec![ExtensionEvent::Detached]
    );
    // Nothing in Anarlog's window resumes a pause, so the recording it keeps is not left paused.
    assert_eq!(
        fake.capture(&cap).map(|view| view.status),
        Some(RecorderStatus::Recording)
    );
    assert_eq!(
        fake.calls(),
        vec![
            "pause:capture-1".to_string(),
            "detach:capture-1".into(),
            "resume:capture-1".into()
        ]
    );

    // The tab's own recording stops instead; it isn't resumed first.
    let fake = Fake::new();
    let route = new_route(&fake);
    let mut relay = Relay::new(peer());
    relay
        .handle(&route, ExtensionMessage::MeetingStart { url: MEET.into() })
        .await;
    relay.handle(&route, ExtensionMessage::MeetingPause).await;
    assert_eq!(
        relay.close(&route).await,
        vec![ExtensionEvent::Ended],
        "closing tells the tab how it was let go"
    );
    assert!(!fake.calls().iter().any(|call| call.starts_with("resume")));
    assert!(fake.world().captures.is_empty());
}

#[tokio::test]
async fn the_tab_hears_paused_only_once_the_recorder_acknowledged_it() {
    let gate = Arc::new(Notify::new());
    let fake = Fake {
        pause_gate: Some(gate.clone()),
        ..Fake::new()
    };
    let cap = fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let route = Arc::new(new_route(&fake));
    let token = route.start(MEET, &peer()).await.unwrap().link;
    let pausing = tokio::spawn({
        let route = route.clone();
        let token = token.clone();
        async move { route.command(&token, Command::Pause).await }
    });
    while !fake.calls().iter().any(|call| call.starts_with("pause")) {
        tokio::task::yield_now().await;
    }
    // Still recording until the recorder says otherwise; no UI-only pause.
    assert_eq!(route.poll(&token), Ok(Status::Recording));
    gate.notify_one();
    assert_eq!(pausing.await.unwrap(), Ok(Status::Paused));
    assert_eq!(route.poll(&token), Ok(Status::Paused));

    // A recorder that can't pause is never asked to.
    fake.world().captures.get_mut(&cap).unwrap().pausable = false;
    assert_eq!(
        route
            .command(&token, Command::Resume)
            .await
            .unwrap_err()
            .code,
        RefusalCode::Unsupported
    );
    assert!(!fake.calls().iter().any(|call| call.starts_with("resume")));
}

async fn read_event(stream: &mut (impl tokio::io::AsyncRead + Unpin)) -> Option<serde_json::Value> {
    use tokio::io::AsyncReadExt;
    let read = async {
        let mut len = [0u8; 4];
        stream.read_exact(&mut len).await.ok()?;
        let mut body = vec![0u8; u32::from_le_bytes(len) as usize];
        stream.read_exact(&mut body).await.ok()?;
        serde_json::from_slice(&body).ok()
    };
    tokio::time::timeout(std::time::Duration::from_secs(2), read)
        .await
        .expect("the app neither answered nor hung up")
}

#[cfg(unix)]
async fn connect_and_start(record: &EndpointRecord) -> tokio::net::UnixStream {
    use tokio::io::AsyncWriteExt;
    let mut stream = tokio::net::UnixStream::connect(&record.socket)
        .await
        .unwrap();
    let hello = Hello {
        protocol: PROTOCOL_VERSION,
        secret: record.secret.clone(),
        origin: ORIGIN.into(),
        browser_pid: Some(CHROME_PID),
    };
    stream
        .write_all(&encode_frame(&hello).unwrap())
        .await
        .unwrap();
    assert_eq!(read_event(&mut stream).await.unwrap()["type"], "welcome");
    stream
        .write_all(&encode_frame(&json!({"type": "meeting_start", "url": MEET})).unwrap())
        .await
        .unwrap();
    stream
}

#[cfg(unix)]
fn endpoint_record(dir: &std::path::Path) -> EndpointRecord {
    serde_json::from_slice(&std::fs::read(dir.join("com.hyprnote.dev.json")).unwrap()).unwrap()
}

/// Turning the bridge off, or quitting, closes connections that were already open: each lets go
/// of its tab by owner, tells the tab, and hangs up, rather than outliving the endpoint.
#[cfg(unix)]
#[tokio::test]
async fn closing_the_endpoint_lets_open_connections_go_by_owner_and_hangs_up() {
    // Short directory so the socket path stays inside `sun_path`.
    let dir = tempfile::Builder::new()
        .prefix("bm")
        .tempdir_in("/tmp")
        .unwrap();
    let endpoints = dir.path().join(ENDPOINT_DIR_NAME);

    // The tab's own recording: the connection stops it and the tab hears it ended.
    let fake = Fake::new();
    let endpoint = serve(
        &endpoints,
        "com.hyprnote.dev",
        vec![ORIGIN.into()],
        Arc::new(new_route(&fake)),
    )
    .unwrap();
    let record = endpoint_record(&endpoints);
    let mut tab = connect_and_start(&record).await;
    assert_eq!(read_event(&mut tab).await.unwrap()["event"], "started");
    assert!(endpoint.close().await, "every connection let go in time");
    assert_eq!(read_event(&mut tab).await.unwrap()["event"], "ended");
    assert_eq!(read_event(&mut tab).await, None, "the app hung up");
    assert!(fake.world().captures.is_empty());
    assert_eq!(
        fake.calls(),
        vec![
            format!("start:{MEET}"),
            "detach:capture-1".into(),
            "stop:capture-1".into()
        ]
    );
    assert!(
        tokio::net::UnixStream::connect(&record.socket)
            .await
            .is_err()
    );

    // A recording Anarlog's window started keeps going; dropping the endpoint closes the same way.
    let fake = Fake::new();
    let cap = fake.app_recording(RecorderStatus::Recording, Some(chrome(Some(MEET))));
    let endpoint = serve(
        &endpoints,
        "com.hyprnote.dev",
        vec![ORIGIN.into()],
        Arc::new(new_route(&fake)),
    )
    .unwrap();
    let mut tab = connect_and_start(&endpoint_record(&endpoints)).await;
    assert_eq!(read_event(&mut tab).await.unwrap()["event"], "started");
    drop(endpoint);
    assert_eq!(read_event(&mut tab).await.unwrap()["event"], "detached");
    assert_eq!(read_event(&mut tab).await, None);
    assert!(fake.capture(&cap).is_some());
    assert_eq!(fake.calls(), vec!["detach:capture-1".to_string()]);
    assert!(fake.world().links.is_empty());
}

/// Closing while a tab's start still waits on the recorder cancels it: what it began stops, the
/// tab is told nothing was linked, and the next click may start again.
#[cfg(unix)]
#[tokio::test]
async fn closing_during_a_start_cancels_it_and_stops_what_it_began() {
    let dir = tempfile::Builder::new()
        .prefix("bm")
        .tempdir_in("/tmp")
        .unwrap();
    let endpoints = dir.path().join(ENDPOINT_DIR_NAME);
    let gate = Arc::new(Notify::new());
    let fake = Fake {
        start_gate: Some(gate.clone()),
        ..Fake::new()
    };
    let route = Arc::new(new_route(&fake));
    let endpoint = serve(
        &endpoints,
        "com.hyprnote.dev",
        vec![ORIGIN.into()],
        route.clone(),
    )
    .unwrap();
    let mut tab = connect_and_start(&endpoint_record(&endpoints)).await;
    while fake.calls().is_empty() {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(endpoint.close().await);
    assert_eq!(
        read_event(&mut tab).await,
        None,
        "nothing linked, nothing to say"
    );
    assert_eq!(
        fake.calls(),
        vec![format!("start:{MEET}"), "cancel:capture-1".into()]
    );
    assert!(fake.world().captures.is_empty());
    // A start that is let go clears the in-flight guard too.
    gate.notify_one();
    assert!(route.start(MEET, &peer()).await.is_ok());
}
