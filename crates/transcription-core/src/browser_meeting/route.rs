use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use serde_json::Value;

use super::call::{browser_family_of, is_canonical_meet_call, meet_call_of, meeting_code_of};
use super::cues::MeetCueReader;
use super::outcome::{CameraOutcome, ChatOutcome};
use super::port::{
    BrowserMeetingPort, DetachOutcome, ExtensionStart, Owner, PortError, RecorderStatus,
    RecorderView,
};

/// A link not heard from in this long belongs to a native host that died without letting go; a
/// click in a Meet tab on the same call may take the recording over. The relay asks every second.
pub const LINK_STALE_MS: u64 = 15_000;

const ENDED: &str = "The recording ended before this tab could follow it.";
const OTHER_MEET_CALL: &str =
    "Anarlog is recording a different Meet call. Stop it in Anarlog to record this one.";
const CANT_PAUSE: &str = "This recording can't be paused here. Stop it from the menu.";
const RETIRED: &str = "Open Anarlog, then click the extension again.";

/// The tab that asks, as its native host told the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub origin: String,
    pub browser_pid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Starting,
    Recording,
    Paused,
    Stopping,
    Ended,
    Detached,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalCode {
    AppNotReady,
    BadRequest,
    /// Another tab holds the recording now, or this one never did.
    NotLinked,
    OtherCall,
    CallUnconfirmed,
    LinkedElsewhere,
    AlreadyRecording,
    Unsupported,
    RecordingFailed,
    /// The recording ended before the tab could link to it.
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: RefusalCode,
    pub message: String,
}

fn refuse<T>(code: RefusalCode, message: &str) -> Result<T, Refusal> {
    Err(Refusal {
        code,
        message: message.to_owned(),
    })
}

fn lost<T>() -> Result<T, Refusal> {
    refuse(
        RefusalCode::NotLinked,
        "This Meet tab isn't linked to Anarlog's recording.",
    )
}

/// The tab's hold on a recording, as the start or link answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linked {
    pub session_id: String,
    pub link: String,
    pub status: Status,
    /// The tab follows a recording Anarlog's window started.
    pub attached: bool,
    pub pausable: bool,
    /// Handed out once: the notice to post and the camera to pick in this call.
    pub notice: Option<String>,
    pub camera: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Pause,
    Resume,
    Stop,
}

/// Whether a Meet operation for the extension may be handed out: never handed, being claimed,
/// handed to that link with no outcome yet (so it may still be going out), an outcome that changed
/// nothing, or one that may have.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Handout {
    None,
    Claiming,
    Pending(String),
    Retry,
    Done,
}

impl Handout {
    fn handable(&self) -> bool {
        matches!(self, Self::None | Self::Retry)
    }
}

/// Meet's hold on one capture. `url` is the call it follows, kept across links so a later click on
/// another call can't move it. At most one `link` at a time, which only that tab's relay knows.
struct Hold {
    owner: Owner,
    url: String,
    link: Option<String>,
    seen_at: u64,
    notice: Handout,
    camera: Handout,
    cues: MeetCueReader,
}

impl Hold {
    fn new(owner: Owner, url: &str) -> Self {
        Self {
            owner,
            url: url.to_owned(),
            link: None,
            seen_at: 0,
            notice: Handout::None,
            camera: Handout::None,
            cues: MeetCueReader::default(),
        }
    }
}

struct State<C> {
    holds: HashMap<C, Hold>,
    /// An extension start in flight: the recorder may not show its capture yet.
    starting: bool,
    /// The bridge this route served was turned off: nothing it served acts again.
    retired: bool,
}

fn lock<C>(state: &Mutex<State<C>>) -> MutexGuard<'_, State<C>> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Clears the in-flight start however it ends, including a dropped connection.
struct Starting<'a, C>(&'a Mutex<State<C>>);

impl<C> Drop for Starting<'_, C> {
    fn drop(&mut self) {
        lock(self.0).starting = false;
    }
}

/// Answers the extension's start, pause, resume, stop, detach, status, cues and outcomes over the
/// app's one recorder and runtime. A start with nothing recording begins a recording the record
/// button's way; with a recording Anarlog's window started, it links the tab to that recording, but
/// only when the recorder confirmed this very Meet call in the browser the tab asks from. Audio is
/// still the whole browser's: the address binds the cues, not the sound, to one tab. Every later
/// request names its link, so a stale or unrelated tab can't act on a recording it doesn't hold.
pub struct BrowserMeetingRoute<P: BrowserMeetingPort> {
    port: P,
    state: Mutex<State<P::Capture>>,
    /// Held across a pause or resume until the recorder acknowledged it, and by a detach before it
    /// looks at the pause, so letting go never misses a pause still landing.
    toggling: tokio::sync::Mutex<()>,
}

impl<P: BrowserMeetingPort> BrowserMeetingRoute<P> {
    pub fn new(port: P) -> Self {
        Self {
            port,
            state: Mutex::new(State {
                holds: HashMap::new(),
                starting: false,
                retired: false,
            }),
            toggling: tokio::sync::Mutex::new(()),
        }
    }

    pub fn port(&self) -> &P {
        &self.port
    }

    fn lock(&self) -> MutexGuard<'_, State<P::Capture>> {
        lock(&self.state)
    }

    fn live(&self, hold: &Hold) -> bool {
        hold.link.is_some() && self.port.now_ms().saturating_sub(hold.seen_at) < LINK_STALE_MS
    }

    pub async fn start(&self, url: &str, peer: &Peer) -> Result<Linked, Refusal> {
        let Some(account_id) = self.port.account() else {
            return refuse(
                RefusalCode::AppNotReady,
                "Sign in to Anarlog to record meetings.",
            );
        };
        if !is_canonical_meet_call(url) {
            return refuse(
                RefusalCode::BadRequest,
                "Open a Google Meet call, then click Anarlog to record it.",
            );
        }
        let followed = {
            let mut state = self.lock();
            if state.retired {
                return refuse(RefusalCode::AppNotReady, RETIRED);
            }
            // Holds from captures that are over, stale, or another account's go.
            state
                .holds
                .retain(|capture, _| self.port.capture(capture).is_some());
            if state.starting {
                return refuse(
                    RefusalCode::LinkedElsewhere,
                    "Anarlog is starting a recording from another Meet tab.",
                );
            }
            match self.port.active() {
                Some(active) => Some(self.follow(&mut state, active, url, peer)?),
                None => {
                    state.starting = true;
                    None
                }
            }
        };
        let (capture, link) = match followed {
            Some(followed) => followed,
            None => {
                let _starting = Starting(&self.state);
                self.start_new(url, peer, account_id).await?
            }
        };
        self.linked(capture, link).await
    }

    /// Links a tab to the active recording, or says why it can't. Runs under the lock, so of two
    /// clicks racing only one links.
    fn follow(
        &self,
        state: &mut State<P::Capture>,
        active: RecorderView<P::Capture>,
        url: &str,
        peer: &Peer,
    ) -> Result<(P::Capture, String), Refusal> {
        if let RecorderStatus::Failed { .. } = active.status {
            return refuse(
                RefusalCode::AlreadyRecording,
                "Anarlog is already recording a meeting. Stop it first.",
            );
        }
        if let Some(held) = state.holds.get(&active.capture) {
            if held.url != url {
                return refuse(RefusalCode::OtherCall, OTHER_MEET_CALL);
            }
            if self.live(held) {
                return refuse(
                    RefusalCode::LinkedElsewhere,
                    "Anarlog is following this recording in another Meet tab.",
                );
            }
        }
        // Also for a takeover of the extension's own recording: the new tab must be on the call
        // the recorder confirmed, in the same browser.
        self.follow_refusal(&active, url, peer)?;
        let link = self.port.link(&active.capture, url).or_else(link_refusal)?;
        let now = self.port.now_ms();
        let hold = state
            .holds
            .entry(active.capture.clone())
            .or_insert_with(|| Hold::new(active.owner, url));
        hold.link = Some(link.clone());
        hold.seen_at = now;
        hold.cues = MeetCueReader::default();
        Ok((active.capture, link))
    }

    /// Why a Meet tab on `url` can't follow the active recording, or nothing when it may: the
    /// recorder must have confirmed this very Meet call, in the browser the tab asks from. The
    /// browser process says only which browser asks; the address picks the call.
    fn follow_refusal(
        &self,
        active: &RecorderView<P::Capture>,
        url: &str,
        peer: &Peer,
    ) -> Result<(), Refusal> {
        if active.status == RecorderStatus::Starting {
            return refuse(
                RefusalCode::CallUnconfirmed,
                "Anarlog is still starting its recording. Click Anarlog again once it's recording.",
            );
        }
        let Some(recorded) = self.port.recorded_call(&active.capture) else {
            return refuse(
                RefusalCode::CallUnconfirmed,
                "Anarlog can't tell which call it's recording, so this tab can't follow it. Stop the recording in Anarlog, then click Anarlog here to record this call.",
            );
        };
        let Some(browser) = browser_family_of(&recorded.bundle_id) else {
            return refuse(
                RefusalCode::OtherCall,
                "Anarlog is recording a call in another app. Stop it in Anarlog to record this Meet call.",
            );
        };
        let Some(call) = recorded.call_url.as_deref().and_then(meet_call_of) else {
            return refuse(
                RefusalCode::CallUnconfirmed,
                "Anarlog can't tell which Meet call it's recording, so this tab can't follow it. Stop the recording in Anarlog, then click Anarlog here to record this call.",
            );
        };
        let asking = peer
            .browser_pid
            .filter(|pid| *pid > 1)
            .and_then(|pid| self.port.bundle_id_of(pid))
            .and_then(|bundle| browser_family_of(&bundle));
        let Some(asking) = asking else {
            return refuse(
                RefusalCode::CallUnconfirmed,
                "Anarlog can't tell which browser this Meet tab is in, so it can't follow the recording. Update Anarlog and the extension together, then click Anarlog again.",
            );
        };
        if asking != browser {
            return refuse(
                RefusalCode::OtherCall,
                "Anarlog is recording a call in another browser. Stop it in Anarlog to record this Meet call.",
            );
        }
        if call != url {
            return refuse(RefusalCode::OtherCall, OTHER_MEET_CALL);
        }
        Ok(())
    }

    /// Starts a recording the record button's way, then links the tab once the recorder confirmed
    /// this Meet call. A recording the tab started whose call isn't confirmed stops again: the
    /// extension's request never becomes the confirmed call.
    async fn start_new(
        &self,
        url: &str,
        peer: &Peer,
        account_id: String,
    ) -> Result<(P::Capture, String), Refusal> {
        let browser_bundle_id = peer
            .browser_pid
            .filter(|pid| *pid > 1)
            .and_then(|pid| self.port.bundle_id_of(pid));
        let started = self
            .port
            .start(ExtensionStart {
                call_url: url.to_owned(),
                browser_pid: peer.browser_pid,
                browser_bundle_id,
                account_id: account_id.clone(),
            })
            .await;
        let capture = match started {
            Ok(capture) => capture,
            Err(PortError::Failed(message)) => {
                return Err(Refusal {
                    code: RefusalCode::RecordingFailed,
                    message,
                });
            }
            Err(_) => {
                return refuse(
                    RefusalCode::RecordingFailed,
                    "Anarlog couldn't record this meeting.",
                );
            }
        };
        let current = self
            .port
            .capture(&capture)
            .filter(|_| self.port.account().as_deref() == Some(account_id.as_str()));
        let linked = match current {
            None => Err(PortError::Stale),
            Some(view) => self
                .follow_refusal(&view, url, peer)
                .map_err(|_| PortError::CallMismatch)
                .and_then(|()| self.port.link(&capture, url)),
        };
        let link = match linked {
            Ok(link) => link,
            Err(error) => {
                let _ = self.port.stop(&capture).await;
                return match error {
                    PortError::Stale => refuse(RefusalCode::Ended, ENDED),
                    _ => refuse(
                        RefusalCode::CallUnconfirmed,
                        "Anarlog couldn't confirm it was recording this Meet call, so it stopped. Start the recording in Anarlog instead.",
                    ),
                };
            }
        };
        let mut hold = Hold::new(Owner::Extension, url);
        hold.link = Some(link.clone());
        hold.seen_at = self.port.now_ms();
        let retired = {
            let mut state = self.lock();
            if !state.retired {
                state.holds.insert(capture.clone(), hold);
            }
            state.retired
        };
        if retired {
            // Turned off meanwhile: nothing would let go of this link, so the tab's own recording
            // stops now.
            let _ = self.port.detach(&link);
            let _ = self.port.stop(&capture).await;
            return refuse(RefusalCode::AppNotReady, RETIRED);
        }
        Ok((capture, link))
    }

    /// Hands the tab its link with the notice and camera the recording hasn't had yet, unless it is
    /// paused. Checked again after the claims: a newer click or the recording's end may have taken
    /// the link meanwhile, and a claim nobody received is released as cancelled before dispatch.
    async fn linked(&self, capture: P::Capture, link: String) -> Result<Linked, Refusal> {
        let Some(view) = self.port.capture(&capture) else {
            return refuse(RefusalCode::Ended, ENDED);
        };
        let handing = matches!(
            view.status,
            RecorderStatus::Starting | RecorderStatus::Recording
        );
        let (claim_notice, claim_camera) = {
            let mut state = self.lock();
            let Some(hold) = state.holds.get_mut(&capture) else {
                return lost();
            };
            let notice = handing && hold.notice.handable();
            let camera = handing && hold.camera.handable();
            if notice {
                hold.notice = Handout::Claiming;
            }
            if camera {
                hold.camera = Handout::Claiming;
            }
            (notice, camera)
        };
        let notice = if claim_notice {
            self.port.claim_notice(&capture).await
        } else {
            None
        };
        let camera = if claim_camera {
            self.port.claim_camera(&capture).await
        } else {
            None
        };
        let owner = {
            let mut state = self.lock();
            let current = !state.retired && self.port.capture(&capture).is_some();
            match state.holds.get_mut(&capture) {
                Some(hold) => {
                    let ours = current && hold.link.as_deref() == Some(link.as_str());
                    for (claimed, handout, given) in [
                        (claim_notice, &mut hold.notice, notice.is_some()),
                        (claim_camera, &mut hold.camera, camera.is_some()),
                    ] {
                        if claimed {
                            *handout = if ours && given {
                                Handout::Pending(link.clone())
                            } else {
                                Handout::None
                            };
                        }
                    }
                    ours.then_some(hold.owner)
                }
                None => None,
            }
        };
        let Some(owner) = owner else {
            if notice.is_some() {
                self.port.notice_outcome(&capture, ChatOutcome::Cancelled);
            }
            if camera.is_some() {
                self.port.camera_outcome(&capture, CameraOutcome::Cancelled);
            }
            return lost();
        };
        Ok(Linked {
            status: status_of(&view),
            session_id: view.session_id,
            link,
            attached: owner == Owner::App,
            pausable: view.pausable,
            notice,
            camera,
        })
    }

    /// The capture this link holds, refreshing its freshness; `None` once the recording is over.
    /// The runtime's own token registry is the authority on stale tabs, so it is asked too. A
    /// retired route answers only letting go.
    fn resolve(&self, link: &str) -> Result<Option<P::Capture>, Refusal> {
        if self.lock().retired {
            return lost();
        }
        self.resolve_held(link)
    }

    fn resolve_held(&self, link: &str) -> Result<Option<P::Capture>, Refusal> {
        let mut state = self.lock();
        let Some(capture) = state
            .holds
            .iter()
            .find(|(_, hold)| hold.link.as_deref() == Some(link))
            .map(|(capture, _)| capture.clone())
        else {
            return lost();
        };
        if self.port.capture(&capture).is_none() {
            state.holds.remove(&capture);
            return Ok(None);
        }
        let now = self.port.now_ms();
        let Some(hold) = state.holds.get_mut(&capture) else {
            return lost();
        };
        if self.port.resolve_link(link).ok().as_ref() != Some(&capture) {
            hold.link = None;
            return lost();
        }
        hold.seen_at = now;
        Ok(Some(capture))
    }

    fn status(&self, capture: &P::Capture) -> Status {
        self.port
            .capture(capture)
            .map_or(Status::Ended, |view| status_of(&view))
    }

    pub fn poll(&self, link: &str) -> Result<Status, Refusal> {
        Ok(match self.resolve(link)? {
            Some(capture) => self.status(&capture),
            None => Status::Ended,
        })
    }

    pub async fn command(&self, link: &str, command: Command) -> Result<Status, Refusal> {
        // Taken before the link is checked, so a pause can't land after a detach let go.
        let _toggling = match command {
            Command::Pause | Command::Resume => Some(self.toggling.lock().await),
            Command::Stop => None,
        };
        let Some(capture) = self.resolve(link)? else {
            return Ok(Status::Ended);
        };
        let Some(view) = self.port.capture(&capture) else {
            return Ok(Status::Ended);
        };
        // A stop that lands after the recording already ended has nothing left to do.
        if command == Command::Stop && view.status == RecorderStatus::Finalizing {
            return Ok(Status::Stopping);
        }
        if command != Command::Stop && !view.pausable {
            return refuse(RefusalCode::Unsupported, CANT_PAUSE);
        }
        let result = match command {
            Command::Pause => self.port.pause(&capture).await,
            Command::Resume => self.port.resume(&capture).await,
            Command::Stop => self.port.stop(&capture).await,
        };
        match result {
            Ok(()) => Ok(self.status(&capture)),
            Err(PortError::Stale) => Ok(Status::Ended),
            Err(PortError::Unsupported) => refuse(RefusalCode::Unsupported, CANT_PAUSE),
            Err(PortError::Failed(message)) => Err(Refusal {
                code: RefusalCode::RecordingFailed,
                message,
            }),
            Err(PortError::CallMismatch) => refuse(
                RefusalCode::RecordingFailed,
                "Anarlog couldn't change the recording.",
            ),
        }
    }

    /// The tab lets go. An app recording keeps going; an extension recording stops with its tab.
    pub async fn detach(&self, link: &str) -> Result<Status, Refusal> {
        let Some(capture) = self.resolve_held(link)? else {
            return Ok(Status::Ended);
        };
        // The runtime ends exactly this token, so a newer tab's link to the recording is untouched.
        let outcome = self.port.detach(link);
        if let Some(hold) = self.lock().holds.get_mut(&capture) {
            hold.link = None;
        }
        match outcome {
            Ok(DetachOutcome::KeepRecording) => {
                // Anarlog's window has no resume, so a pause made from this tab ends with it,
                // including one the recorder is still acknowledging.
                let _toggling = self.toggling.lock().await;
                let paused = self
                    .port
                    .capture(&capture)
                    .is_some_and(|view| view.status == RecorderStatus::Paused);
                if paused && let Err(error) = self.port.resume(&capture).await {
                    tracing::warn!(?error, "browser_meeting_resume_on_detach_failed");
                }
                Ok(Status::Detached)
            }
            Err(_) => Ok(Status::Detached),
            Ok(DetachOutcome::StopRecording) => match self.port.stop(&capture).await {
                Ok(()) | Err(PortError::Stale) => Ok(Status::Ended),
                Err(PortError::Failed(message)) => Ok(Status::Failed(message)),
                Err(_) => Ok(Status::Failed(
                    "Anarlog couldn't stop the recording. Stop it in Anarlog.".into(),
                )),
            },
        }
    }

    /// The bridge is turned off and its connections closed: lets go of every link this route still
    /// holds, by owner, then refuses everything else. Only this route's own tokens are let go, so a
    /// newer bridge's tab keeps its link. Idempotent.
    pub async fn retire(&self) {
        let links: Vec<String> = {
            let mut state = self.lock();
            state.retired = true;
            state
                .holds
                .values()
                .filter_map(|hold| hold.link.clone())
                .collect()
        };
        for link in links {
            let _ = self.detach(&link).await;
        }
    }

    /// What Meet showed about who is here and speaking, from the linked tab only.
    pub fn cues(&self, link: &str, events: &[Value]) -> Result<(), Refusal> {
        let Some(capture) = self.resolve(link)? else {
            return lost();
        };
        let observations = {
            let mut state = self.lock();
            let Some(hold) = state.holds.get_mut(&capture) else {
                return lost();
            };
            let code = meeting_code_of(&hold.url).unwrap_or_default().to_owned();
            hold.cues.read(&code, events).map_err(|_| Refusal {
                code: RefusalCode::BadRequest,
                message: "These cues aren't from this call.".into(),
            })?
        };
        for observation in observations {
            self.port.observe(&capture, observation);
        }
        Ok(())
    }

    /// What the notice handed to this link came to. Only that link may say, and only once.
    pub fn chat(&self, link: &str, outcome: &str) -> Result<(), Refusal> {
        let Some(outcome) = ChatOutcome::parse(outcome) else {
            return refuse(
                RefusalCode::BadRequest,
                "Update Anarlog and the extension together.",
            );
        };
        let Some(capture) = self.resolve(link)? else {
            return lost();
        };
        {
            let mut state = self.lock();
            let Some(hold) = state.holds.get_mut(&capture) else {
                return lost();
            };
            if hold.notice != Handout::Pending(link.to_owned()) {
                return refuse(
                    RefusalCode::NotLinked,
                    "This Meet tab wasn't asked to post Anarlog's notice.",
                );
            }
            hold.notice = if outcome.may_have_sent() {
                Handout::Done
            } else {
                Handout::Retry
            };
        }
        self.port.notice_outcome(&capture, outcome);
        Ok(())
    }

    /// What the camera switch handed to this link came to.
    pub fn camera(&self, link: &str, outcome: &str) -> Result<(), Refusal> {
        let Some(capture) = self.resolve(link)? else {
            return lost();
        };
        let parsed = CameraOutcome::parse(outcome);
        {
            let mut state = self.lock();
            let Some(hold) = state.holds.get_mut(&capture) else {
                return lost();
            };
            if hold.camera != Handout::Pending(link.to_owned()) {
                return refuse(
                    RefusalCode::NotLinked,
                    "This Meet tab wasn't asked to switch the camera.",
                );
            }
            hold.camera = if parsed.is_some_and(CameraOutcome::untouched) {
                Handout::Retry
            } else {
                Handout::Done
            };
        }
        if let Some(outcome) = parsed {
            self.port.camera_outcome(&capture, outcome);
        }
        Ok(())
    }
}

fn link_refusal<T>(error: PortError) -> Result<T, Refusal> {
    match error {
        PortError::CallMismatch => refuse(
            RefusalCode::CallUnconfirmed,
            "Anarlog couldn't confirm it is recording this Meet call.",
        ),
        PortError::Stale => refuse(RefusalCode::Ended, ENDED),
        PortError::Unsupported | PortError::Failed(_) => refuse(
            RefusalCode::RecordingFailed,
            "Anarlog couldn't link this Meet tab.",
        ),
    }
}

fn status_of<C>(view: &RecorderView<C>) -> Status {
    match &view.status {
        RecorderStatus::Starting => Status::Starting,
        RecorderStatus::Recording => Status::Recording,
        RecorderStatus::Paused => Status::Paused,
        RecorderStatus::Finalizing => Status::Stopping,
        RecorderStatus::Failed { message } => Status::Failed(message.clone()),
    }
}
