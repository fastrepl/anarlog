use std::panic::AssertUnwindSafe;

use objc2::rc::Retained;
use objc2_foundation::NSPredicate;

use crate::types::ParticipantContact;

pub trait ContactFetcher: Send + Sync {
    fn fetch_contact_with_predicate(&self, predicate: &NSPredicate) -> Option<ParticipantContact>;
}

pub fn resolve_participant_contact(
    participant: &objc2_event_kit::EKParticipant,
    url: Option<&str>,
    name: Option<&str>,
    contact_fetcher: Option<&dyn ContactFetcher>,
) -> (Option<String>, Option<ParticipantContact>) {
    if let Some(contact) = contact_fetcher.and_then(|f| try_fetch_contact(participant, f)) {
        // prefer the event url email if it exists
        // in order to get the calendar-relevant email
        // (e.g. work google account)
        let email = parse_email_from_url(url)
            .filter(|e| {
                contact
                    .email_addresses
                    .iter()
                    .any(|ce| ce.eq_ignore_ascii_case(e))
            })
            .or_else(|| contact.email_addresses.first().cloned());
        if email.is_some() {
            return (email, Some(contact));
        }
        let email = parse_email_from_url(url).or_else(|| parse_email_from_name(name));
        return (email, Some(contact));
    }

    let email = parse_email_from_url(url).or_else(|| parse_email_from_name(name));
    (email, None)
}

fn try_fetch_contact(
    participant: &objc2_event_kit::EKParticipant,
    fetcher: &dyn ContactFetcher,
) -> Option<ParticipantContact> {
    let participant = AssertUnwindSafe(participant);
    let predicate: Retained<NSPredicate> =
        match unsafe { objc2::exception::catch(|| participant.contactPredicate()) } {
            Ok(p) => p,
            Err(_) => return None,
        };

    fetcher.fetch_contact_with_predicate(&predicate)
}

fn parse_email_from_url(url: Option<&str>) -> Option<String> {
    let url = url?;
    let lower = url.to_lowercase();
    if lower.starts_with("mailto:") {
        let email = url[7..].to_string();
        if !email.is_empty() {
            return Some(email);
        }
    }
    None
}

fn parse_email_from_name(name: Option<&str>) -> Option<String> {
    let name = name?.trim();
    if name.contains('@') && name.contains('.') && !name.contains(' ') {
        Some(name.to_string())
    } else {
        None
    }
}
