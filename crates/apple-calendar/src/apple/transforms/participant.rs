use objc2::{msg_send, rc::Retained};
use objc2_event_kit::EKParticipant;
use objc2_foundation::NSURL;

use crate::types::Participant;

use super::super::contacts::{self, ContactFetcher};
use super::enums::{
    transform_participant_role, transform_participant_status, transform_participant_type,
};

pub fn transform_participant(
    participant: &EKParticipant,
    contact_fetcher: Option<&dyn ContactFetcher>,
) -> Participant {
    let name = unsafe { participant.name() }.map(|s| s.to_string());

    let is_current_user = unsafe { participant.isCurrentUser() };
    let role = transform_participant_role(unsafe { participant.participantRole() });
    let status = transform_participant_status(unsafe { participant.participantStatus() });
    let participant_type = transform_participant_type(unsafe { participant.participantType() });

    let url = unsafe {
        let url_obj: Option<Retained<NSURL>> = msg_send![participant, URL];
        url_obj.and_then(|u| u.absoluteString().map(|s| s.to_string()))
    };

    let (email, contact) = contacts::resolve_participant_contact(
        participant,
        url.as_deref(),
        name.as_deref(),
        contact_fetcher,
    );

    Participant {
        name,
        email,
        is_current_user,
        role,
        status,
        participant_type,
        url,
        contact,
    }
}
