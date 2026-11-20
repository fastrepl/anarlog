/// What the extension's chat notice came to, as `meeting-chat.js` resolves it. Meet taking the
/// message, never proof that anyone read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatOutcome {
    Sent,
    /// This call's chat was marked sent by an earlier recording in the same tab.
    AlreadySent,
    /// An earlier send in this call that Meet never confirmed.
    AlreadyAttempted,
    /// Sent, but Meet didn't empty its input in time: it may be in the chat.
    Unconfirmed,
    NotInCall,
    ChatUnavailable,
    DraftInProgress,
    NotSent,
    /// The recording stopped before anything was typed or sent.
    Cancelled,
}

impl ChatOutcome {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "sent" => Self::Sent,
            "already_sent" => Self::AlreadySent,
            "already_attempted" => Self::AlreadyAttempted,
            "unconfirmed" => Self::Unconfirmed,
            "not_in_call" => Self::NotInCall,
            "chat_unavailable" => Self::ChatUnavailable,
            "draft_in_progress" => Self::DraftInProgress,
            "not_sent" => Self::NotSent,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }

    /// Whether the notice may be in the chat. Once true, it is never handed out again.
    pub fn may_have_sent(self) -> bool {
        self.kind() == NoticeKind::Sent || self.kind() == NoticeKind::Unconfirmed
    }

    /// The shared notice result (`anlg_audio::meeting::NoticeOutcome`): sent, definitely not sent,
    /// cancelled before dispatch, or unconfirmed after dispatch.
    pub fn kind(self) -> NoticeKind {
        match self {
            Self::Sent | Self::AlreadySent => NoticeKind::Sent,
            Self::AlreadyAttempted | Self::Unconfirmed => NoticeKind::Unconfirmed,
            Self::NotInCall | Self::ChatUnavailable | Self::DraftInProgress | Self::NotSent => {
                NoticeKind::NotSent
            }
            Self::Cancelled => NoticeKind::CancelledBeforeDispatch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Sent,
    NotSent,
    CancelledBeforeDispatch,
    Unconfirmed,
}

/// What the extension's camera switch in Meet's own settings came to (`meeting-camera.js`). It only
/// picks which camera Meet uses; a camera that is off stays off and Meet's effects stay manual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraOutcome {
    Switched,
    AlreadyUsing,
    /// An earlier recording in this call already tried; the person's choice stands.
    AlreadyTried,
    CameraMissing,
    PickerUnavailable,
    /// Picked, but Meet's picker never showed it.
    Unconfirmed,
    NotInCall,
    Cancelled,
}

impl CameraOutcome {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "switched" => Self::Switched,
            "already_using" => Self::AlreadyUsing,
            "already_tried" => Self::AlreadyTried,
            "camera_missing" => Self::CameraMissing,
            "picker_unavailable" => Self::PickerUnavailable,
            "unconfirmed" => Self::Unconfirmed,
            "not_in_call" => Self::NotInCall,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }

    /// Meet's camera wasn't touched, so a later tab may try again. Every other outcome may have
    /// opened Meet's settings and is not retried.
    pub fn untouched(self) -> bool {
        matches!(self, Self::NotInCall | Self::Cancelled)
    }
}
