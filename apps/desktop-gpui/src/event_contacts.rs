//! `participants/event-contact-extraction.ts`: the participant chip's
//! `Enhance contact` — contacts inferred from the calendar event's text and
//! attendees, and the durable changes planned for one human.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization as _;

const MAX_EVENT_TEXT_CHARS: usize = 6000;
const MAX_CONTACTS_TO_EXTRACT: usize = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Candidate {
    pub human_id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_current_user: bool,
    pub is_organizer: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub title: Option<String>,
    pub description: Option<String>,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    pub email: Option<String>,
    pub company_name: Option<String>,
}

/// `ApplyContactEnhancementResult`
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub created: u32,
    pub updated: u32,
    pub linked: u32,
    pub skipped: u32,
    pub contacts: Vec<Contact>,
    pub matched: bool,
}

/// `ContactEnhancementChanges`
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub name: Option<String>,
    pub email: Option<String>,
    pub company_name: Option<String>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.email.is_none() && self.company_name.is_none()
    }
}

/// A session participant row as the context reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParticipantRecord {
    pub human_id: String,
    pub name: String,
    pub email: String,
    pub source: String,
}

/// A calendar event attendee (`EventParticipant`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attendee {
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_current_user: bool,
    pub is_organizer: bool,
}

/// The human a plan targets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HumanRecord {
    pub name: String,
    pub email: String,
    pub organization_id: String,
}

/// `buildEventContactExtractionContextFromRecords`
pub fn build_context(
    title: Option<&str>,
    description: Option<&str>,
    current_user_id: &str,
    participants: &[ParticipantRecord],
    attendees: &[Attendee],
) -> Context {
    let mut candidates: Vec<Candidate> = participants
        .iter()
        .filter(|participant| participant.source != "excluded")
        .map(|participant| Candidate {
            human_id: Some(participant.human_id.clone()),
            name: Some(participant.name.clone()),
            email: Some(participant.email.clone()),
            is_current_user: participant.human_id == current_user_id,
            is_organizer: false,
        })
        .collect();
    candidates.extend(attendees.iter().map(|attendee| Candidate {
        human_id: None,
        name: attendee.name.clone(),
        email: attendee.email.clone(),
        is_current_user: attendee.is_current_user,
        is_organizer: attendee.is_organizer,
    }));
    Context {
        title: title.map(str::to_string),
        description: description.map(str::to_string),
        candidates: dedupe_candidates(candidates),
    }
}

/// `planExtractedContactToHuman`
pub fn plan_for_human(
    human_id: &str,
    user_id: &str,
    human: Option<&HumanRecord>,
    current_user: Option<&HumanRecord>,
    mapping_source: Option<&str>,
    participant: Option<(&str, &str)>,
    contacts: &[Contact],
) -> (Outcome, Changes) {
    let normalized = normalize_extracted_contacts(
        contacts
            .iter()
            .map(|contact| RawContact {
                name: Some(contact.name.clone()),
                email: contact.email.clone(),
                company_name: contact.company_name.clone(),
            })
            .collect(),
        &[],
    );
    let mut result = Outcome::default();
    let mut changes = Changes::default();

    match mapping_source {
        None | Some("") | Some("excluded") => {
            if !normalized.is_empty() {
                result.skipped += 1;
            }
            return (result, changes);
        }
        _ => {}
    }

    let identity_name = human
        .map(|h| h.name.clone())
        .filter(|name| !name.is_empty())
        .or_else(|| participant.map(|(name, _)| name.to_string()))
        .unwrap_or_default();
    let identity_email = human
        .map(|h| h.email.clone())
        .filter(|email| !email.is_empty())
        .or_else(|| participant.map(|(_, email)| email.to_string()))
        .unwrap_or_default();
    let identity = Candidate {
        name: Some(identity_name.clone()),
        email: Some(identity_email.clone()),
        ..Candidate::default()
    };
    let contact = find_contact_for_human(&identity, &normalized)
        .cloned()
        .or_else(|| contact_from_identity(&identity_name, &identity_email));

    let Some(contact) = contact else {
        if human_id == user_id
            && let Some(first) = normalized.first()
        {
            result.matched = true;
            result.contacts.push(first.clone());
            result.skipped += 1;
        }
        return (result, changes);
    };

    result.matched = true;
    result.contacts.push(contact.clone());
    if human_id == user_id || is_current_user_contact(&contact, current_user) {
        result.skipped += 1;
        return (result, changes);
    }

    let Some(human) = human else {
        changes.name = Some(contact.name.clone());
        changes.email = contact.email.clone();
        changes.company_name = contact.company_name.clone();
        result.created = 1;
        return (result, changes);
    };

    if should_update_human_name(&human.name, contact.email.as_deref()) {
        changes.name = Some(contact.name.clone());
    }
    if should_update_human_email(&human.email, contact.email.as_deref()) {
        changes.email = contact.email.clone();
    }
    if human.organization_id.is_empty() && contact.company_name.is_some() {
        changes.company_name = contact.company_name.clone();
    }
    if !changes.is_empty() {
        result.updated = 1;
    }
    (result, changes)
}

/// `extractEventContacts`
pub fn extract_contacts(context: &Context) -> Vec<Contact> {
    let mut raw = infer_contacts_from_event_text(context);
    raw.extend(
        context
            .candidates
            .iter()
            .filter(|candidate| !candidate.is_current_user)
            .map(|candidate| RawContact {
                name: candidate.name.clone(),
                email: candidate.email.clone(),
                company_name: None,
            }),
    );
    normalize_extracted_contacts(raw, &context.candidates)
}

fn dedupe_candidates(candidates: Vec<Candidate>) -> Vec<Candidate> {
    // `Map` insertion order: a re-inserted key keeps its slot.
    let mut order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, Candidate> = HashMap::new();
    for candidate in candidates {
        let email = normalize_email(candidate.email.as_deref());
        let human_id = candidate
            .human_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let name = clean_name_hint(candidate.name.as_deref().unwrap_or(""));
        let key = if let Some(email) = &email {
            format!("email:{email}")
        } else if let Some(human_id) = human_id {
            format!("human:{human_id}")
        } else if !name.is_empty() {
            format!("name:{}", normalize_name(&name))
        } else {
            continue;
        };
        let existing = by_key.get(&key).cloned();
        let merged = Candidate {
            // `{ ...existing, ...candidate }`: the new candidate's fields win,
            // except the ones spelled out after.
            human_id: candidate
                .human_id
                .clone()
                .or_else(|| existing.as_ref().and_then(|e| e.human_id.clone())),
            name: existing
                .as_ref()
                .and_then(|e| e.name.clone())
                .filter(|n| !n.is_empty())
                .or_else(|| (!name.is_empty()).then(|| name.clone()))
                .or(candidate.name.clone()),
            email: existing
                .as_ref()
                .and_then(|e| e.email.clone())
                .filter(|n| !n.is_empty())
                .or(candidate.email.clone()),
            is_current_user: existing.as_ref().is_some_and(|e| e.is_current_user)
                || candidate.is_current_user,
            is_organizer: existing.as_ref().is_some_and(|e| e.is_organizer)
                || candidate.is_organizer,
        };
        if existing.is_none() {
            order.push(key.clone());
        }
        by_key.insert(key, merged);
    }
    order
        .into_iter()
        .filter_map(|key| by_key.remove(&key))
        .collect()
}

fn trim_event_text(value: Option<&str>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    // `.slice(0, 6000)` counts UTF-16 units.
    let stripped = strip_html(value);
    let trimmed = stripped.trim();
    let mut units = 0usize;
    let mut end = trimmed.len();
    for (index, ch) in trimmed.char_indices() {
        let width = ch.len_utf16();
        if units + width > MAX_EVENT_TEXT_CHARS {
            end = index;
            break;
        }
        units += width;
    }
    trimmed[..end].to_string()
}

struct RawContact {
    name: Option<String>,
    email: Option<String>,
    company_name: Option<String>,
}

// JS `\w` is ASCII.
static LABEL_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_\s]+:\s*").unwrap());
static ANGLE_EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*@[^>]*>").unwrap());
static ROLE_PARENS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\([^)]*(organizer|host|required|optional)[^)]*\)").unwrap());
static BETWEEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bbetween\s+(.+?)(?:\s+(?:at|on|for)\b|$)").unwrap());
static NAME_DELIMITER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*(?:<>|<->)\s*|\s+\band\b\s+|\s+&\s+").unwrap());
static LINE_BREAKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n+").unwrap());

fn infer_contacts_from_event_text(context: &Context) -> Vec<RawContact> {
    let mut order: Vec<String> = Vec::new();
    let mut contacts: HashMap<String, String> = HashMap::new();
    let lines: Vec<String> = [context.title.as_deref(), context.description.as_deref()]
        .into_iter()
        .flat_map(|value| {
            LINE_BREAKS
                .split(&trim_event_text(value))
                .map(|line| line.trim().to_string())
                .collect::<Vec<_>>()
        })
        .filter(|line| !line.is_empty())
        .collect();

    let mut add_name = |value: &str| {
        let without_label = LABEL_PREFIX.replace(value, "");
        let without_email = ANGLE_EMAIL.replace_all(&without_label, "");
        let without_role = ROLE_PARENS.replace_all(&without_email, "");
        let name = clean_name_hint(&without_role);
        let key = normalize_name(&name);
        if key.is_empty()
            || !is_likely_person_name(&name)
            || is_self_reference(&name, &context.candidates)
        {
            return;
        }
        if !contacts.contains_key(&key) {
            order.push(key.clone());
        }
        contacts.insert(key, name);
    };

    for line in &lines {
        if let Some(captures) = BETWEEN.captures(line)
            && let Some(names) = captures.get(1)
        {
            add_delimited_names(names.as_str(), &mut add_name);
            continue;
        }
        if line.contains("<>") {
            add_delimited_names(line, &mut add_name);
        }
    }

    order
        .into_iter()
        .filter_map(|key| contacts.remove(&key))
        .map(|name| RawContact {
            name: Some(name),
            email: None,
            company_name: None,
        })
        .collect()
}

fn add_delimited_names(value: &str, add_name: &mut impl FnMut(&str)) {
    for part in NAME_DELIMITER.split(value) {
        add_name(part);
    }
}

static BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
static CLOSE_P: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</p>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static NBSP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&nbsp;").unwrap());
static AMP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&amp;").unwrap());
static LT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&lt;").unwrap());
static GT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&gt;").unwrap());

fn strip_html(value: &str) -> String {
    let value = BR.replace_all(value, "\n");
    let value = CLOSE_P.replace_all(&value, "\n");
    let value = TAG.replace_all(&value, " ");
    let value = NBSP.replace_all(&value, " ");
    let value = AMP.replace_all(&value, "&");
    let value = LT.replace_all(&value, "<");
    let value = GT.replace_all(&value, ">");
    value.replace('\u{00a0}', " ")
}

fn normalize_extracted_contacts(
    contacts: Vec<RawContact>,
    candidates: &[Candidate],
) -> Vec<Contact> {
    let mut order: Vec<String> = Vec::new();
    let mut deduped: HashMap<String, Contact> = HashMap::new();
    let mut keys_by_name: HashMap<String, String> = HashMap::new();

    fn set(
        order: &mut Vec<String>,
        deduped: &mut HashMap<String, Contact>,
        key: String,
        contact: Contact,
    ) {
        if !deduped.contains_key(&key) {
            order.push(key.clone());
        }
        deduped.insert(key, contact);
    }
    fn delete(order: &mut Vec<String>, deduped: &mut HashMap<String, Contact>, key: &str) {
        if deduped.remove(key).is_some() {
            order.retain(|k| k != key);
        }
    }

    for contact in contacts {
        let mut name = clean_name_hint(contact.name.as_deref().unwrap_or(""));
        let email =
            normalize_email(contact.email.as_deref()).or_else(|| normalize_email(Some(&name)));
        if !is_likely_person_name(&name)
            && let Some(email) = &email
        {
            name = name_from_email_local_part(email);
        }
        if !is_likely_person_name(&name)
            || (email.is_none() && is_self_reference(&name, candidates))
        {
            continue;
        }

        let matched_email = email.or_else(|| match_candidate_email(&name, candidates));
        let company_name = normalize_company_name(contact.company_name.as_deref())
            .or_else(|| infer_company_name_from_email(matched_email.as_deref()));
        let normalized = Contact {
            name: name.clone(),
            email: matched_email.clone(),
            company_name,
        };
        if is_self_contact_from_candidates(&normalized, candidates) {
            continue;
        }

        let name_key = normalize_name(&name);
        let key = match &matched_email {
            Some(email) => format!("email:{email}"),
            None => format!("name:{name_key}"),
        };
        let existing_key = if deduped.contains_key(&key) {
            Some(key.clone())
        } else {
            keys_by_name.get(&name_key).cloned()
        };
        let existing = existing_key.as_ref().and_then(|k| deduped.get(k).cloned());
        let Some(existing) = existing else {
            set(&mut order, &mut deduped, key.clone(), normalized);
            keys_by_name.insert(name_key, key);
            continue;
        };

        let existing_email = normalize_email(existing.email.as_deref());
        if let (Some(matched), Some(existing_email)) = (&matched_email, &existing_email)
            && existing_email != matched
        {
            if let Some(existing_key) = &existing_key {
                delete(&mut order, &mut deduped, existing_key);
            }
            set(&mut order, &mut deduped, key.clone(), normalized);
            keys_by_name.insert(name_key, key);
            continue;
        }

        let merged = Contact {
            name: existing.name.clone(),
            email: existing.email.clone().or(normalized.email.clone()),
            company_name: existing
                .company_name
                .clone()
                .or(normalized.company_name.clone()),
        };
        let merged_key = match &merged.email {
            Some(email) => format!("email:{email}"),
            None => format!("name:{name_key}"),
        };
        if let Some(existing_key) = &existing_key
            && *existing_key != merged_key
        {
            delete(&mut order, &mut deduped, existing_key);
        }
        set(&mut order, &mut deduped, merged_key.clone(), merged);
        keys_by_name.insert(name_key, merged_key);
    }

    order
        .into_iter()
        .filter_map(|key| deduped.remove(&key))
        .take(MAX_CONTACTS_TO_EXTRACT)
        .collect()
}

static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static LEADING_MARKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[#>*\-\s]+").unwrap());
static ROLE_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+-\s+(organizer|host|required|optional)$").unwrap());
static LEADING_QUOTES: LazyLock<Regex> = LazyLock::new(|| Regex::new("^[\"'`]+").unwrap());
static TRAILING_PUNCT: LazyLock<Regex> = LazyLock::new(|| Regex::new("[\"'`,.;:]+$").unwrap());

fn clean_name_hint(value: &str) -> String {
    let value = WHITESPACE.replace_all(value, " ");
    let value = LEADING_MARKS.replace(&value, "");
    let value = ROLE_SUFFIX.replace(&value, "");
    let value = LEADING_QUOTES.replace(&value, "");
    let value = TRAILING_PUNCT.replace(&value, "");
    value.trim().to_string()
}

static URL_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^https?://").unwrap());

fn is_likely_person_name(value: &str) -> bool {
    let length = value.encode_utf16().count();
    if value.is_empty() || !(2..=80).contains(&length) {
        return false;
    }
    if value.contains('@') || URL_PREFIX.is_match(value) {
        return false;
    }
    let normalized = normalize_name(value);
    if normalized.is_empty()
        || [
            "what",
            "who",
            "invitee timezone",
            "meeting link",
            "zoom",
            "google meet",
            "teams",
        ]
        .contains(&normalized.as_str())
    {
        return false;
    }
    value.chars().filter(|ch| ch.is_alphabetic()).count() >= 2
}

static EMAIL_SEPARATORS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[._+\-]+").unwrap());

fn email_local_tokens(email: &str) -> Vec<String> {
    let local = email.split('@').next().unwrap_or("");
    tokenize_name(&EMAIL_SEPARATORS.replace_all(local, " "))
}

fn match_candidate_email(name: &str, candidates: &[Candidate]) -> Option<String> {
    let name_tokens = tokenize_name(name);
    if name_tokens.is_empty() {
        return None;
    }
    let mut best: Option<(String, f64)> = None;
    let mut first_name_matches: Vec<String> = Vec::new();
    let first_name_token = &name_tokens[0];

    for candidate in candidates {
        let Some(email) = normalize_email(candidate.email.as_deref()) else {
            continue;
        };
        if candidate.is_current_user {
            continue;
        }
        let mut candidate_tokens = tokenize_name(candidate.name.as_deref().unwrap_or(""));
        candidate_tokens.extend(email_local_tokens(&email));
        if candidate_tokens.is_empty() {
            continue;
        }
        let matched: Vec<&String> = name_tokens
            .iter()
            .filter(|token| candidate_tokens.contains(token))
            .collect();
        let score = matched.len() as f64 / name_tokens.len() as f64;
        let enough_signal = if name_tokens.len() == 1 {
            score == 1.0
        } else {
            score >= 0.67 || matched.len() >= 2
        };
        if enough_signal
            && best
                .as_ref()
                .is_none_or(|(_, best_score)| score > *best_score)
        {
            best = Some((email, score));
        } else if name_tokens.len() > 1 && matched.len() == 1 && matched[0] == first_name_token {
            first_name_matches.push(email);
        }
    }

    best.map(|(email, _)| email)
        .or_else(|| (first_name_matches.len() == 1).then(|| first_name_matches[0].clone()))
}

fn is_self_reference(value: &str, candidates: &[Candidate]) -> bool {
    let normalized = normalize_name(value);
    if normalized.is_empty() {
        return false;
    }
    candidates
        .iter()
        .filter(|candidate| candidate.is_current_user)
        .any(|candidate| candidate_aliases(candidate).contains(&normalized))
}

fn is_self_contact_from_candidates(contact: &Contact, candidates: &[Candidate]) -> bool {
    let email = normalize_email(contact.email.as_deref());
    let name = normalize_name(&contact.name);
    candidates
        .iter()
        .filter(|candidate| candidate.is_current_user)
        .any(|candidate| {
            let candidate_email = normalize_email(candidate.email.as_deref());
            match &email {
                Some(email) => candidate_email.as_deref() == Some(email.as_str()),
                None => candidate_aliases(candidate).contains(&name),
            }
        })
}

fn is_current_user_contact(contact: &Contact, current_user: Option<&HumanRecord>) -> bool {
    let Some(current_user) = current_user else {
        return false;
    };
    let email = normalize_email(contact.email.as_deref());
    let current_email = normalize_email(string_cell(&current_user.email));
    if let (Some(email), Some(current_email)) = (&email, &current_email) {
        return email == current_email;
    }
    let candidate = Candidate {
        name: string_cell(&current_user.name).map(str::to_string),
        email: string_cell(&current_user.email).map(str::to_string),
        is_current_user: true,
        ..Candidate::default()
    };
    candidate_aliases(&candidate).contains(&normalize_name(&contact.name))
}

fn find_contact_for_human<'a>(human: &Candidate, contacts: &'a [Contact]) -> Option<&'a Contact> {
    let human_email = normalize_email(human.email.as_deref());
    let human_name = normalize_name(human.name.as_deref().unwrap_or(""));
    let human_aliases = strong_candidate_aliases(human);
    contacts.iter().find(|contact| {
        let contact_email = normalize_email(contact.email.as_deref());
        if let Some(human_email) = &human_email
            && contact_email.as_deref() == Some(human_email.as_str())
        {
            return true;
        }
        let contact_name = normalize_name(&contact.name);
        if !contact_name.is_empty() && human_aliases.contains(&contact_name) {
            return true;
        }
        let contact_aliases = strong_candidate_aliases(&Candidate {
            name: Some(contact.name.clone()),
            email: contact.email.clone(),
            ..Candidate::default()
        });
        !human_name.is_empty() && contact_aliases.contains(&human_name)
    })
}

fn candidate_aliases(candidate: &Candidate) -> Vec<String> {
    let mut aliases = strong_candidate_aliases(candidate);
    let name_tokens = tokenize_name(candidate.name.as_deref().unwrap_or(""));
    if let Some(first) = name_tokens.first() {
        aliases.push(first.clone());
    }
    if let Some(local) = candidate
        .email
        .as_deref()
        .and_then(|email| email.split('@').next())
    {
        let spaced = EMAIL_SEPARATORS.replace_all(local, " ");
        let email_tokens = tokenize_name(&spaced);
        let normalized_local = normalize_name(&spaced);
        if !normalized_local.is_empty() {
            aliases.push(normalized_local);
        }
        if let Some(first) = email_tokens.first() {
            aliases.push(first.clone());
        }
    }
    aliases
}

fn strong_candidate_aliases(candidate: &Candidate) -> Vec<String> {
    let mut aliases = Vec::new();
    let normalized = normalize_name(candidate.name.as_deref().unwrap_or(""));
    if !normalized.is_empty() {
        aliases.push(normalized);
    }
    if let Some(local) = candidate
        .email
        .as_deref()
        .and_then(|email| email.split('@').next())
    {
        let normalized_local = normalize_name(&EMAIL_SEPARATORS.replace_all(local, " "));
        if !normalized_local.is_empty() {
            aliases.push(normalized_local);
        }
    }
    aliases
}

const PERSONAL_EMAIL_DOMAINS: &[&str] = &[
    "gmail.com",
    "googlemail.com",
    "yahoo.com",
    "outlook.com",
    "hotmail.com",
    "live.com",
    "msn.com",
    "icloud.com",
    "me.com",
    "mac.com",
    "aol.com",
    "proton.me",
    "protonmail.com",
    "pm.me",
    "hey.com",
    "fastmail.com",
];

fn contact_from_identity(name: &str, email: &str) -> Option<Contact> {
    let raw_name = clean_name_hint(name);
    let email = normalize_email(Some(email)).or_else(|| normalize_email(Some(&raw_name)));
    let name = if is_likely_person_name(&raw_name) {
        raw_name
    } else if let Some(email) = &email {
        name_from_email_local_part(email)
    } else {
        String::new()
    };
    if !is_likely_person_name(&name) {
        return None;
    }
    let company_name = infer_company_name_from_email(email.as_deref());
    Some(Contact {
        name,
        email,
        company_name,
    })
}

static LOCAL_SEPARATORS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[._\-]+").unwrap());

fn name_from_email_local_part(email: &str) -> String {
    let local = email
        .split('@')
        .next()
        .and_then(|local| local.split('+').next())
        .unwrap_or("");
    LOCAL_SEPARATORS
        .replace_all(local, " ")
        .split(' ')
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.chars().all(|ch| ch.is_ascii_digit()))
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn infer_company_name_from_email(email: Option<&str>) -> Option<String> {
    let domain = email?.split('@').nth(1)?.to_lowercase();
    if domain.is_empty() || PERSONAL_EMAIL_DOMAINS.contains(&domain.as_str()) {
        return None;
    }
    let labels: Vec<&str> = domain
        .split('.')
        .filter(|label| !label.is_empty())
        .collect();
    if labels.len() < 2 {
        return None;
    }
    let second_last = labels[labels.len() - 2];
    let company_label =
        if labels.len() >= 3 && ["co", "com", "org", "net", "ac"].contains(&second_last) {
            labels[labels.len() - 3]
        } else {
            second_last
        };
    if company_label.encode_utf16().count() < 2 {
        return None;
    }
    let mut chars = company_label.chars();
    let capitalized = match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => return None,
    };
    normalize_company_name(Some(&capitalized))
}

fn should_update_human_name(existing_name: &str, email: Option<&str>) -> bool {
    let current = existing_name.trim();
    if current.is_empty() {
        return true;
    }
    if let Some(email) = email
        && normalize_email(Some(current)) == normalize_email(Some(email))
    {
        return true;
    }
    current.contains('@')
}

fn should_update_human_email(existing_email: &str, email: Option<&str>) -> bool {
    normalize_email(email).is_some() && normalize_email(Some(existing_email)).is_none()
}

static EMAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap());

fn normalize_email(value: Option<&str>) -> Option<String> {
    let email = value?.trim().to_lowercase();
    (!email.is_empty() && EMAIL.is_match(&email)).then_some(email)
}

fn normalize_company_name(value: Option<&str>) -> Option<String> {
    let name = WHITESPACE.replace_all(value?.trim(), " ").to_string();
    let length = name.encode_utf16().count();
    if name.is_empty() || !(2..=80).contains(&length) {
        return None;
    }
    if name.contains('@') || URL_PREFIX.is_match(&name) {
        return None;
    }
    Some(name)
}

static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

/// `normalizeName`: NFKD without combining marks, lower-case, runs of
/// anything but `a-z0-9` collapsed to one space.
fn normalize_name(value: &str) -> String {
    let decomposed: String = value
        .nfkd()
        .filter(|ch| !('\u{0300}'..='\u{036f}').contains(ch))
        .collect();
    NON_ALNUM
        .replace_all(&decomposed.to_lowercase(), " ")
        .trim()
        .to_string()
}

fn tokenize_name(value: &str) -> Vec<String> {
    normalize_name(value)
        .split(' ')
        .filter(|token| token.encode_utf16().count() > 1)
        .map(str::to_string)
        .collect()
}

fn string_cell(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> (Option<&'static str>, Option<&'static str>) {
        (Some("Alice Kim <> John"), Some("Alice Kim from Example"))
    }

    fn participant(human_id: &str, name: &str, email: &str, source: &str) -> ParticipantRecord {
        ParticipantRecord {
            human_id: human_id.into(),
            name: name.into(),
            email: email.into(),
            source: source.into(),
        }
    }

    fn contact(name: &str, email: Option<&str>, company: Option<&str>) -> Contact {
        Contact {
            name: name.into(),
            email: email.map(str::to_string),
            company_name: company.map(str::to_string),
        }
    }

    #[test]
    fn builds_context_from_participants_and_attendees() {
        let (title, description) = event();
        let context = build_context(
            title,
            description,
            "user-1",
            &[
                participant("human-1", "Alice", "alice@example.com", "manual"),
                participant(
                    "human-excluded",
                    "Excluded",
                    "excluded@example.com",
                    "excluded",
                ),
            ],
            &[Attendee {
                name: Some("Bob".into()),
                email: Some("bob@example.com".into()),
                is_organizer: true,
                ..Attendee::default()
            }],
        );
        assert_eq!(context.candidates.len(), 2);
        assert_eq!(context.candidates[0].human_id.as_deref(), Some("human-1"));
        assert_eq!(context.candidates[0].name.as_deref(), Some("Alice"));
        assert_eq!(context.candidates[1].name.as_deref(), Some("Bob"));
        assert!(context.candidates[1].is_organizer);
    }

    #[test]
    fn deduplicates_a_participant_and_its_attendee() {
        let (title, description) = event();
        let context = build_context(
            title,
            description,
            "user-1",
            &[participant("human-1", "Alice", "alice@example.com", "auto")],
            &[Attendee {
                name: Some("Alice Kim".into()),
                email: Some("ALICE@example.com".into()),
                is_organizer: true,
                ..Attendee::default()
            }],
        );
        assert_eq!(context.candidates.len(), 1);
        let candidate = &context.candidates[0];
        assert_eq!(candidate.human_id.as_deref(), Some("human-1"));
        assert_eq!(candidate.email.as_deref(), Some("alice@example.com"));
        assert!(candidate.is_organizer);
    }

    #[test]
    fn plans_durable_fields() {
        let human = HumanRecord {
            name: "Alice Kim".into(),
            email: String::new(),
            organization_id: String::new(),
        };
        let current = HumanRecord {
            name: "John".into(),
            email: "john@example.com".into(),
            organization_id: String::new(),
        };
        let (result, changes) = plan_for_human(
            "human-1",
            "user-1",
            Some(&human),
            Some(&current),
            Some("manual"),
            None,
            &[contact(
                "Alice Kim",
                Some("alice@example.com"),
                Some("Example"),
            )],
        );
        assert!(result.matched);
        assert_eq!((result.updated, result.skipped), (1, 0));
        assert_eq!(
            changes,
            Changes {
                name: None,
                email: Some("alice@example.com".into()),
                company_name: Some("Example".into()),
            }
        );
    }

    #[test]
    fn skips_an_excluded_participant() {
        let human = HumanRecord {
            name: "Alice".into(),
            ..HumanRecord::default()
        };
        let (result, changes) = plan_for_human(
            "human-1",
            "user-1",
            Some(&human),
            None,
            Some("excluded"),
            None,
            &[contact("Alice", Some("alice@example.com"), None)],
        );
        assert!(!result.matched);
        assert_eq!(result.skipped, 1);
        assert!(changes.is_empty());
    }

    #[test]
    fn never_overwrites_the_current_user() {
        let john = HumanRecord {
            name: "John".into(),
            email: "john@example.com".into(),
            organization_id: String::new(),
        };
        let (result, changes) = plan_for_human(
            "user-1",
            "user-1",
            Some(&john),
            Some(&john),
            Some("manual"),
            None,
            &[contact("John Jeong", Some("john@example.com"), None)],
        );
        assert!(result.matched);
        assert_eq!((result.skipped, result.updated), (1, 0));
        assert!(changes.is_empty());
    }

    #[test]
    fn extracts_from_event_text_and_candidates() {
        let (title, description) = event();
        let context = build_context(
            title,
            description,
            "user-1",
            &[
                participant("human-1", "Alice Kim", "alice@example.com", "manual"),
                participant("user-1", "John", "john@example.com", "manual"),
            ],
            &[],
        );
        assert_eq!(
            extract_contacts(&context),
            vec![contact(
                "Alice Kim",
                Some("alice@example.com"),
                Some("Example")
            )]
        );
    }

    #[test]
    fn creates_a_contact_from_the_participant_when_the_human_is_missing() {
        let current = HumanRecord {
            name: "John Jeong".into(),
            email: "john@example.com".into(),
            organization_id: String::new(),
        };
        let (result, changes) = plan_for_human(
            "human-1",
            "user-1",
            None,
            Some(&current),
            Some("auto"),
            Some(("marco.bambini@gmail.com", "marco.bambini@gmail.com")),
            &[],
        );
        assert!(result.matched);
        assert_eq!((result.created, result.updated), (1, 0));
        assert_eq!(
            changes,
            Changes {
                name: Some("Marco Bambini".into()),
                email: Some("marco.bambini@gmail.com".into()),
                company_name: None,
            }
        );
    }

    #[test]
    fn derives_a_name_from_an_email_only_human() {
        let human = HumanRecord {
            name: "marco.bambini@gmail.com".into(),
            email: "marco.bambini@gmail.com".into(),
            organization_id: String::new(),
        };
        let current = HumanRecord {
            name: "John Jeong".into(),
            email: "john@example.com".into(),
            organization_id: String::new(),
        };
        let (result, changes) = plan_for_human(
            "human-1",
            "user-1",
            Some(&human),
            Some(&current),
            Some("auto"),
            None,
            &[],
        );
        assert!(result.matched);
        assert_eq!((result.created, result.updated), (0, 1));
        assert_eq!(
            changes,
            Changes {
                name: Some("Marco Bambini".into()),
                email: None,
                company_name: None,
            }
        );
    }

    #[test]
    fn infers_a_company_from_a_work_email() {
        let context = Context {
            title: Some("Check-in".into()),
            description: None,
            candidates: vec![
                Candidate {
                    name: Some("John Jeong".into()),
                    email: Some("john@example.com".into()),
                    is_current_user: true,
                    ..Candidate::default()
                },
                Candidate {
                    name: Some("tom@kestroll.com".into()),
                    email: Some("tom@kestroll.com".into()),
                    ..Candidate::default()
                },
            ],
        };
        let contacts = extract_contacts(&context);
        let current = HumanRecord {
            name: "John Jeong".into(),
            email: "john@example.com".into(),
            organization_id: String::new(),
        };
        let (result, changes) = plan_for_human(
            "human-1",
            "user-1",
            None,
            Some(&current),
            Some("auto"),
            Some(("tom@kestroll.com", "tom@kestroll.com")),
            &contacts,
        );
        assert!(result.matched);
        assert_eq!(result.created, 1);
        assert_eq!(
            changes,
            Changes {
                name: Some("Tom".into()),
                email: Some("tom@kestroll.com".into()),
                company_name: Some("Kestroll".into()),
            }
        );
    }

    #[test]
    fn a_shared_first_name_with_another_email_is_not_the_current_user() {
        let context = Context {
            title: Some("Intro".into()),
            description: None,
            candidates: vec![
                Candidate {
                    name: Some("John Jeong".into()),
                    email: Some("john@example.com".into()),
                    is_current_user: true,
                    ..Candidate::default()
                },
                Candidate {
                    name: Some("john@other.com".into()),
                    email: Some("john@other.com".into()),
                    ..Candidate::default()
                },
            ],
        };
        let contacts = extract_contacts(&context);
        assert_eq!(
            contacts,
            vec![contact("John", Some("john@other.com"), Some("Other"))]
        );
        let current = HumanRecord {
            name: "John Jeong".into(),
            email: "john@example.com".into(),
            organization_id: String::new(),
        };
        let (result, changes) = plan_for_human(
            "human-1",
            "user-1",
            None,
            Some(&current),
            Some("auto"),
            Some(("john@other.com", "john@other.com")),
            &contacts,
        );
        assert!(result.matched);
        assert_eq!((result.created, result.skipped), (1, 0));
        assert_eq!(
            changes,
            Changes {
                name: Some("John".into()),
                email: Some("john@other.com".into()),
                company_name: Some("Other".into()),
            }
        );
    }

    #[test]
    fn extracts_organizers_that_only_have_an_email() {
        let context = Context {
            title: Some("SQLite AI".into()),
            description: Some(
                "<p>Marco Bambini is inviting you to a scheduled Zoom meeting.</p>".into(),
            ),
            candidates: vec![
                Candidate {
                    human_id: Some("user-1".into()),
                    name: Some("John Jeong".into()),
                    email: Some("john@example.com".into()),
                    is_current_user: true,
                    is_organizer: false,
                },
                Candidate {
                    human_id: Some("human-1".into()),
                    name: Some("marco.bambini@gmail.com".into()),
                    email: Some("marco.bambini@gmail.com".into()),
                    is_current_user: false,
                    is_organizer: true,
                },
            ],
        };
        assert_eq!(
            extract_contacts(&context),
            vec![contact(
                "Marco Bambini",
                Some("marco.bambini@gmail.com"),
                None
            )]
        );
    }

    #[test]
    fn names_between_and_around_delimiters_are_read_from_the_text() {
        let context = Context {
            title: Some("Sync between Ada Lovelace and Lin Zhou at 10".into()),
            description: Some("Host: Grace Hopper (organizer) <> Bob Stone".into()),
            candidates: vec![Candidate {
                name: Some("Me".into()),
                email: Some("me@example.com".into()),
                is_current_user: true,
                ..Candidate::default()
            }],
        };
        let names: Vec<String> = extract_contacts(&context)
            .into_iter()
            .map(|contact| contact.name)
            .collect();
        assert_eq!(
            names,
            ["Ada Lovelace", "Lin Zhou", "Grace Hopper", "Bob Stone"]
        );
    }
}
