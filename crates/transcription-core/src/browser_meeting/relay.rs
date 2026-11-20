use super::port::BrowserMeetingPort;
use super::protocol::{ExtensionEvent, ExtensionMessage};
use super::route::{BrowserMeetingRoute, Command, Peer, Refusal, RefusalCode, Status};

/// The page scripts' outcomes are short names; anything else isn't one.
const MAX_OUTCOME_LENGTH: usize = 32;

struct Link {
    token: String,
    pausable: bool,
    /// Goes out with the first `started`, so the extension acts on each once.
    notice: Option<String>,
    camera: Option<String>,
}

/// One native-host connection: one Meet tab's port, open for the length of its hold. It turns the
/// extension's messages into route requests and tells the extension each change once; the
/// transport polls it every second so a pause or stop made in Anarlog's window reaches the tab
/// too. Messages are handled one at a time, so a stop sent during a start is read once it lands.
pub struct Relay {
    peer: Peer,
    link: Option<Link>,
    last: Status,
}

impl Relay {
    pub fn new(peer: Peer) -> Self {
        Self {
            peer,
            link: None,
            last: Status::Ended,
        }
    }

    pub fn linked(&self) -> bool {
        self.link.is_some()
    }

    pub async fn handle<P: BrowserMeetingPort>(
        &mut self,
        route: &BrowserMeetingRoute<P>,
        message: ExtensionMessage,
    ) -> Vec<ExtensionEvent> {
        match message {
            ExtensionMessage::MeetingStart { url } => {
                if self.link.is_some() {
                    return vec![];
                }
                match route.start(&url, &self.peer).await {
                    Ok(linked) => {
                        self.link = Some(Link {
                            token: linked.link,
                            pausable: linked.pausable,
                            notice: linked.notice,
                            camera: linked.camera,
                        });
                        self.report(linked.status)
                    }
                    Err(refusal) if refusal.code == RefusalCode::Ended => {
                        vec![ExtensionEvent::Ended]
                    }
                    Err(refusal) => vec![ExtensionEvent::Error {
                        message: refusal.message,
                    }],
                }
            }
            ExtensionMessage::MeetingDetach => match self.token() {
                Some(token) => {
                    let status = route.detach(&token).await;
                    self.answer(status)
                }
                None => vec![],
            },
            ExtensionMessage::MeetingCues { events } => {
                if let Some(token) = self.token() {
                    let _ = route.cues(&token, &events);
                }
                vec![]
            }
            // Reports, not commands: one that comes after the link let go is dropped.
            ExtensionMessage::MeetingChat { outcome } => {
                if let Some(token) = self.token()
                    && outcome.len() <= MAX_OUTCOME_LENGTH
                {
                    let _ = route.chat(&token, &outcome);
                }
                vec![]
            }
            ExtensionMessage::MeetingCamera { outcome } => {
                if let Some(token) = self.token()
                    && outcome.len() <= MAX_OUTCOME_LENGTH
                {
                    let _ = route.camera(&token, &outcome);
                }
                vec![]
            }
            ExtensionMessage::MeetingPause
            | ExtensionMessage::MeetingResume
            | ExtensionMessage::MeetingStop => {
                let Some(token) = self.token() else {
                    return vec![ExtensionEvent::Error {
                        message: "Anarlog isn't recording this meeting.".into(),
                    }];
                };
                let command = match message {
                    ExtensionMessage::MeetingPause => Command::Pause,
                    ExtensionMessage::MeetingResume => Command::Resume,
                    _ => Command::Stop,
                };
                let status = route.command(&token, command).await;
                self.answer(status)
            }
        }
    }

    /// Picks up changes made in Anarlog's own window.
    pub fn poll<P: BrowserMeetingPort>(
        &mut self,
        route: &BrowserMeetingRoute<P>,
    ) -> Vec<ExtensionEvent> {
        match self.token() {
            Some(token) => {
                let status = route.poll(&token);
                self.answer(status)
            }
            None => vec![],
        }
    }

    /// The extension went away mid-recording, or the bridge is closing, so nothing in the tab can
    /// stop it on leaving the call: the route stops a recording the tab started and lets go of one
    /// it joined. Returns what to tell the tab, if it can still hear.
    pub async fn close<P: BrowserMeetingPort>(
        &mut self,
        route: &BrowserMeetingRoute<P>,
    ) -> Vec<ExtensionEvent> {
        let events = match self.token() {
            Some(token) => {
                let status = route.detach(&token).await;
                self.answer(status)
            }
            None => vec![],
        };
        self.finish();
        events
    }

    fn token(&self) -> Option<String> {
        self.link.as_ref().map(|link| link.token.clone())
    }

    fn finish(&mut self) {
        self.link = None;
        self.last = Status::Ended;
    }

    fn answer(&mut self, status: Result<Status, Refusal>) -> Vec<ExtensionEvent> {
        match status {
            Ok(status) => self.report(status),
            // Another tab took the recording, or this one never held it: it goes on without us.
            Err(refusal) if refusal.code == RefusalCode::NotLinked => self.report(Status::Detached),
            // The link and the recording stay as they were.
            Err(refusal)
                if matches!(
                    refusal.code,
                    RefusalCode::Unsupported | RefusalCode::RecordingFailed
                ) =>
            {
                vec![ExtensionEvent::Refused {
                    message: refusal.message,
                }]
            }
            Err(refusal) => self.report(Status::Failed(refusal.message)),
        }
    }

    /// Tells the extension each change once; `starting` and `stopping` say nothing.
    fn report(&mut self, status: Status) -> Vec<ExtensionEvent> {
        match status {
            Status::Ended => {
                self.finish();
                vec![ExtensionEvent::Ended]
            }
            Status::Detached => {
                self.finish();
                vec![ExtensionEvent::Detached]
            }
            Status::Failed(message) => {
                self.finish();
                vec![ExtensionEvent::Error { message }]
            }
            status if status == self.last => vec![],
            status => {
                let previous = std::mem::replace(&mut self.last, status.clone());
                match (status, &mut self.link) {
                    (Status::Recording, _) if previous == Status::Paused => {
                        vec![ExtensionEvent::Resumed]
                    }
                    (Status::Recording, Some(link)) => vec![ExtensionEvent::Started {
                        pausable: link.pausable,
                        notice: link.notice.take(),
                        camera: link.camera.take(),
                    }],
                    (Status::Paused, _) => vec![ExtensionEvent::Paused],
                    _ => vec![],
                }
            }
        }
    }
}
