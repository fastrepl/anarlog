//! Google Meet recording from the browser extension, shell-independent.
//!
//! The extension (`browser-meeting-extension/`) holds one native-messaging port per recorded tab.
//! Chrome runs the native host (`crates/chrome-native-host`), which proxies that port to the app's
//! socket after presenting this launch's secret and the extension's origin. In the app, one
//! [`Relay`] per connection drives the [`BrowserMeetingRoute`], which acts only through the
//! [`BrowserMeetingPort`]: the one meeting runtime and the one recorder path. A closed connection
//! lets go as the tab leaving does: a recording the tab started stops, one it joined keeps going.
//!
//! Ported from Char's Meet attachment (`apps/desktop/src/main/online/browser-meeting.ts` and
//! `apps/cli/src/next/browser-meeting.ts`); see `PROVENANCE.md`.

mod call;
mod cues;
mod outcome;
mod port;
mod protocol;
mod relay;
mod route;
mod start;
mod transport;

#[cfg(all(test, unix))]
mod runtime_tests;
#[cfg(test)]
mod tests;

pub use call::{
    browser_family_of, is_canonical_meet_call, meet_call_of, meeting_code_of, same_meet_call,
};
pub use cues::{MeetCueReader, MeetObservation, MeetParticipant, MeetSpeaker};
pub use outcome::{CameraOutcome, ChatOutcome, NoticeKind};
pub use port::{
    BrowserMeetingPort, DetachOutcome, ExtensionStart, Owner, PortError, RecordedCall,
    RecorderStatus, RecorderView,
};
pub use protocol::{
    EndpointRecord, ExtensionEvent, ExtensionMessage, Hello, HelloReply, MAX_FRAME_BYTES,
    PROTOCOL_VERSION, encode_frame,
};
pub use relay::Relay;
pub use route::{
    BrowserMeetingRoute, Command, LINK_STALE_MS, Linked, Peer, Refusal, RefusalCode, Status,
};
pub use start::{
    ExtensionStarts, START_GO_EVENT, START_REPLY_EVENT, START_REQUEST_EVENT, START_TIMEOUT,
    StartGo, StartHost, StartReply, StartRequest, WINDOW_TIMEOUT,
};
pub use transport::{Admission, Closing, ENDPOINT_DIR_NAME, POLL_INTERVAL, serve_connection};
#[cfg(unix)]
pub use transport::{Endpoint, serve};
