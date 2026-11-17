//! Shipping's participant "Enhance contact" (`metadata/participants/event-contact-extraction.ts`
//! and `useEventContactEnhancement`): names and emails from the calendar event's title,
//! description and attendees complete the clicked participant's contact.

use std::sync::LazyLock;

use desktop_runtime::{Result, RuntimeHandle, ServiceError, SessionId};
use regex::Regex;
use serde_json::{Value, json};

use crate::{
    meeting::model::failure,
    workspace::contacts::{
        identity::{
            infer_company_name_from_email, is_likely_person_name, name_from_email_local_part,
            normalize_company_name, normalize_name,
        },
        ops::{self, Write},
    },
};

const MAX_EVENT_TEXT_CHARS: usize = 6000;
const MAX_CONTACTS_TO_EXTRACT: usize = 8;

#[derive(Clone, Debug, Default, PartialEq)]
struct Candidate {
    name: String,
    email: String,
    is_current_user: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Contact {
    name: String,
    email: Option<String>,
    company: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Changes {
    name: Option<String>,
    email: Option<String>,
    company: Option<String>,
}

/// The toast shipping shows for an enhancement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Outcome {
    Created,
    NotFound,
    UpToDate,
    Enhanced,
}

impl Outcome {
    pub(super) fn toast(self) -> crate::application::toasts::Toast {
        use crate::application::toasts::Toast;
        match self {
            Self::Created => Toast::success("Contact created"),
            Self::NotFound => Toast::info("No contact detail found"),
            Self::UpToDate => Toast::info("Contact already up to date"),
            Self::Enhanced => Toast::success("Contact enhanced"),
        }
    }
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("contact pattern")
}

fn normalize_email(value: &str) -> Option<String> {
    static EMAIL: LazyLock<Regex> = LazyLock::new(|| regex(r"^[^\s@]+@[^\s@]+\.[^\s@]+$"));
    let email = value.trim().to_lowercase();
    EMAIL.is_match(&email).then_some(email)
}

/// Shipping's `cleanNameHint`.
fn clean_name_hint(value: &str) -> String {
    static LEADING: LazyLock<Regex> = LazyLock::new(|| regex(r"^[#>*\-\s]+"));
    static ROLE: LazyLock<Regex> =
        LazyLock::new(|| regex(r"(?i)\s+-\s+(organizer|host|required|optional)$"));
    static QUOTES: LazyLock<Regex> = LazyLock::new(|| regex(r#"^["'`]+"#));
    static TRAILING: LazyLock<Regex> = LazyLock::new(|| regex(r#"["'`,.;:]+$"#));
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let value = LEADING.replace(&value, "");
    let value = ROLE.replace(&value, "");
    let value = QUOTES.replace(&value, "");
    TRAILING.replace(&value, "").trim().to_owned()
}

fn tokens(value: &str) -> Vec<String> {
    normalize_name(value)
        .split(' ')
        .filter(|token| token.chars().count() > 1)
        .map(str::to_owned)
        .collect()
}

fn email_local(email: &str) -> String {
    static SEPARATORS: LazyLock<Regex> = LazyLock::new(|| regex(r"[._+-]+"));
    SEPARATORS
        .replace_all(email.split('@').next().unwrap_or_default(), " ")
        .into_owned()
}

/// Shipping's `getStrongCandidateAliases`: the full name and the email's local part.
fn strong_aliases(name: &str, email: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    let name = normalize_name(name);
    if !name.is_empty() {
        aliases.push(name);
    }
    let local = normalize_name(&email_local(email));
    if !email.split('@').next().unwrap_or_default().is_empty() && !local.is_empty() {
        aliases.push(local);
    }
    aliases
}

/// Shipping's `getCandidateAliases`: the strong aliases plus first names.
fn aliases(name: &str, email: &str) -> Vec<String> {
    let mut aliases = strong_aliases(name, email);
    aliases.extend(tokens(name).into_iter().next());
    if !email.split('@').next().unwrap_or_default().is_empty() {
        aliases.extend(tokens(&email_local(email)).into_iter().next());
    }
    aliases
}

fn is_self_reference(value: &str, candidates: &[Candidate]) -> bool {
    let normalized = normalize_name(value);
    !normalized.is_empty()
        && candidates.iter().any(|candidate| {
            candidate.is_current_user
                && aliases(&candidate.name, &candidate.email).contains(&normalized)
        })
}

fn is_self_contact(contact: &Contact, candidates: &[Candidate]) -> bool {
    let email = contact.email.as_deref().and_then(normalize_email);
    let name = normalize_name(&contact.name);
    candidates.iter().any(|candidate| {
        candidate.is_current_user
            && match &email {
                Some(email) => normalize_email(&candidate.email).as_ref() == Some(email),
                None => aliases(&candidate.name, &candidate.email).contains(&name),
            }
    })
}

/// Shipping's `matchCandidateEmail`: an attendee email whose name or address matches.
fn match_candidate_email(name: &str, candidates: &[Candidate]) -> Option<String> {
    let name_tokens = tokens(name);
    let first = name_tokens.first()?;
    let mut best: Option<(String, f64)> = None;
    let mut first_name_matches = Vec::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| !candidate.is_current_user)
    {
        let Some(email) = normalize_email(&candidate.email) else {
            continue;
        };
        let mut candidate_tokens = tokens(&candidate.name);
        candidate_tokens.extend(tokens(&email_local(&email)));
        if candidate_tokens.is_empty() {
            continue;
        }
        let matched: Vec<&String> = name_tokens
            .iter()
            .filter(|token| candidate_tokens.contains(token))
            .collect();
        let score = matched.len() as f64 / name_tokens.len() as f64;
        let enough = if name_tokens.len() == 1 {
            score == 1.
        } else {
            score >= 0.67 || matched.len() >= 2
        };
        if enough && best.as_ref().is_none_or(|(_, best)| score > *best) {
            best = Some((email, score));
        } else if name_tokens.len() > 1 && matched.len() == 1 && matched[0] == first {
            first_name_matches.push(email);
        }
    }
    best.map(|(email, _)| email)
        .or_else(|| (first_name_matches.len() == 1).then(|| first_name_matches.remove(0)))
}

/// Shipping's `normalizeExtractedContacts`: person-like names, matched attendee emails and
/// companies, deduplicated by email then name, without the current user.
fn normalize_contacts(contacts: Vec<Contact>, candidates: &[Candidate]) -> Vec<Contact> {
    // Insertion-ordered like the JavaScript `Map`s shipping uses.
    let mut deduped: Vec<(String, Contact)> = Vec::new();
    let mut keys_by_name: Vec<(String, String)> = Vec::new();
    let set = |deduped: &mut Vec<(String, Contact)>, key: String, contact: Contact| match deduped
        .iter_mut()
        .find(|(existing, _)| *existing == key)
    {
        Some((_, slot)) => *slot = contact,
        None => deduped.push((key, contact)),
    };
    let set_name = |keys: &mut Vec<(String, String)>, name: String, key: String| match keys
        .iter_mut()
        .find(|(existing, _)| *existing == name)
    {
        Some((_, slot)) => *slot = key,
        None => keys.push((name, key)),
    };
    for contact in contacts {
        let mut name = clean_name_hint(&contact.name);
        let email = contact
            .email
            .as_deref()
            .and_then(normalize_email)
            .or_else(|| normalize_email(&name));
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
        let company = normalize_company_name(contact.company.as_deref())
            .or_else(|| infer_company_name_from_email(matched_email.as_deref()));
        let normalized = Contact {
            name: name.clone(),
            email: matched_email.clone(),
            company,
        };
        if is_self_contact(&normalized, candidates) {
            continue;
        }
        let name_key = normalize_name(&name);
        let key = match &matched_email {
            Some(email) => format!("email:{email}"),
            None => format!("name:{name_key}"),
        };
        let existing_key = if deduped.iter().any(|(existing, _)| *existing == key) {
            Some(key.clone())
        } else {
            keys_by_name
                .iter()
                .find(|(existing, _)| *existing == name_key)
                .map(|(_, key)| key.clone())
        };
        let existing = existing_key.as_ref().and_then(|existing_key| {
            deduped
                .iter()
                .find(|(existing, _)| existing == existing_key)
                .map(|(_, contact)| contact.clone())
        });
        let Some(existing) = existing else {
            set(&mut deduped, key.clone(), normalized);
            set_name(&mut keys_by_name, name_key, key);
            continue;
        };
        let existing_key = existing_key.unwrap_or_default();
        let existing_email = existing.email.as_deref().and_then(normalize_email);
        if let (Some(matched), Some(previous)) = (&matched_email, &existing_email)
            && matched != previous
        {
            deduped.retain(|(existing, _)| *existing != existing_key);
            set(&mut deduped, key.clone(), normalized);
            set_name(&mut keys_by_name, name_key, key);
            continue;
        }
        let merged = Contact {
            name: existing.name.clone(),
            email: existing.email.clone().or(normalized.email),
            company: existing.company.clone().or(normalized.company),
        };
        let merged_key = match &merged.email {
            Some(email) => format!("email:{email}"),
            None => format!("name:{name_key}"),
        };
        if existing_key != merged_key {
            deduped.retain(|(existing, _)| *existing != existing_key);
        }
        set(&mut deduped, merged_key.clone(), merged);
        set_name(&mut keys_by_name, name_key, merged_key);
    }
    deduped
        .into_iter()
        .map(|(_, contact)| contact)
        .take(MAX_CONTACTS_TO_EXTRACT)
        .collect()
}

fn strip_html(value: &str) -> String {
    static BREAK: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)<br\s*/?>"));
    static PARAGRAPH: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)</p>"));
    static TAG: LazyLock<Regex> = LazyLock::new(|| regex(r"<[^>]+>"));
    let value = BREAK.replace_all(value, "\n");
    let value = PARAGRAPH.replace_all(&value, "\n");
    let value = TAG.replace_all(&value, " ");
    let mut value = value.into_owned();
    for (entity, text) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
    ] {
        value = regex(&format!("(?i){entity}"))
            .replace_all(&value, text)
            .into_owned();
    }
    value.replace('\u{a0}', " ")
}

/// Shipping's `inferContactsFromEventText`: names in "between A and B" and "A <> B" lines.
fn infer_from_text(title: &str, description: &str, candidates: &[Candidate]) -> Vec<Contact> {
    static LABEL: LazyLock<Regex> = LazyLock::new(|| regex(r"^[A-Za-z0-9_\s]+:\s*"));
    static ADDRESS: LazyLock<Regex> = LazyLock::new(|| regex(r"<[^>]*@[^>]*>"));
    static ROLE: LazyLock<Regex> =
        LazyLock::new(|| regex(r"(?i)\([^)]*(organizer|host|required|optional)[^)]*\)"));
    static BETWEEN: LazyLock<Regex> =
        LazyLock::new(|| regex(r"(?i)(?-u:\b)between\s+(.+?)(?:\s+(?:at|on|for)(?-u:\b)|$)"));
    static DELIMITER: LazyLock<Regex> =
        LazyLock::new(|| regex(r"(?i)\s*(?:<>|<->)\s*|\s+(?-u:\b)and(?-u:\b)\s+|\s+&\s+"));
    static LINES: LazyLock<Regex> = LazyLock::new(|| regex(r"\n+"));
    let mut contacts: Vec<(String, Contact)> = Vec::new();
    let mut add = |value: &str| {
        let value = LABEL.replace(value, "");
        let value = ADDRESS.replace_all(&value, "");
        let name = clean_name_hint(&ROLE.replace_all(&value, ""));
        let key = normalize_name(&name);
        if key.is_empty() || !is_likely_person_name(&name) || is_self_reference(&name, candidates) {
            return;
        }
        let contact = Contact {
            name,
            ..Contact::default()
        };
        match contacts.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, slot)) => *slot = contact,
            None => contacts.push((key, contact)),
        }
    };
    let trimmed = |value: &str| -> String {
        let text: Vec<u16> = strip_html(value).trim().encode_utf16().collect();
        String::from_utf16_lossy(&text[..text.len().min(MAX_EVENT_TEXT_CHARS)])
    };
    for line in [title, description]
        .iter()
        .flat_map(|value| {
            LINES
                .split(&trimmed(value))
                .map(|line| line.trim().to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|line| !line.is_empty())
    {
        if let Some(names) = BETWEEN.captures(&line).and_then(|found| found.get(1)) {
            for part in DELIMITER.split(names.as_str()) {
                add(part);
            }
            continue;
        }
        if line.contains("<>") {
            for part in DELIMITER.split(&line) {
                add(part);
            }
        }
    }
    contacts.into_iter().map(|(_, contact)| contact).collect()
}

/// Shipping's `contactFromIdentity`.
fn contact_from_identity(name: &str, email: &str) -> Option<Contact> {
    let raw = clean_name_hint(name);
    let email = normalize_email(email).or_else(|| normalize_email(&raw));
    let name = if is_likely_person_name(&raw) {
        raw
    } else {
        email
            .as_deref()
            .map(name_from_email_local_part)
            .unwrap_or_default()
    };
    if !is_likely_person_name(&name) {
        return None;
    }
    let company = infer_company_name_from_email(email.as_deref());
    Some(Contact {
        name,
        email,
        company,
    })
}

/// Shipping's `findContactForHuman`.
fn contact_for<'a>(name: &str, email: &str, contacts: &'a [Contact]) -> Option<&'a Contact> {
    let human_aliases = strong_aliases(name.trim(), email.trim());
    let email = normalize_email(email);
    let name_key = normalize_name(name);
    contacts.iter().find(|contact| {
        let contact_email = contact.email.as_deref().and_then(normalize_email);
        if email.is_some() && contact_email == email {
            return true;
        }
        let contact_name = normalize_name(&contact.name);
        (!contact_name.is_empty() && human_aliases.contains(&contact_name))
            || (!name_key.is_empty()
                && strong_aliases(&contact.name, contact.email.as_deref().unwrap_or_default())
                    .contains(&name_key))
    })
}

/// Shipping's `isCurrentUserContact`.
fn is_current_user(contact: &Contact, user: Option<&Person>) -> bool {
    let Some(user) = user else {
        return false;
    };
    let email = contact.email.as_deref().and_then(normalize_email);
    if let (Some(email), Some(current)) = (&email, normalize_email(&user.email)) {
        return *email == current;
    }
    aliases(user.name.trim(), user.email.trim()).contains(&normalize_name(&contact.name))
}

#[derive(Clone, Debug, Default)]
struct Person {
    name: String,
    email: String,
    organization: String,
}

/// Shipping's `planExtractedContactToHuman`: the changes for the clicked participant and the
/// toast they lead to.
fn plan(
    human_id: &str,
    user_id: &str,
    human: Option<&Person>,
    user: Option<&Person>,
    participant: Option<(&str, &Person)>,
    contacts: Vec<Contact>,
) -> (Outcome, Changes, bool) {
    let contacts = normalize_contacts(contacts, &[]);
    let not_found = (Outcome::NotFound, Changes::default(), false);
    let Some((_, participant)) = participant.filter(|(source, _)| *source != "excluded") else {
        return not_found;
    };
    let pick = |own: Option<&str>, fallback: &str| {
        own.filter(|value| !value.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    };
    let name = pick(human.map(|human| human.name.as_str()), &participant.name);
    let email = pick(human.map(|human| human.email.as_str()), &participant.email);
    let Some(contact) = contact_for(&name, &email, &contacts)
        .cloned()
        .or_else(|| contact_from_identity(&name, &email))
    else {
        let matched = human_id == user_id && !contacts.is_empty();
        return if matched {
            (Outcome::UpToDate, Changes::default(), false)
        } else {
            not_found
        };
    };
    if human_id == user_id || is_current_user(&contact, user) {
        return (Outcome::UpToDate, Changes::default(), false);
    }
    let Some(human) = human else {
        return (
            Outcome::Created,
            Changes {
                name: Some(contact.name),
                email: contact.email,
                company: contact.company,
            },
            true,
        );
    };
    let current = human.name.trim();
    let update_name = current.is_empty()
        || contact
            .email
            .as_deref()
            .is_some_and(|email| normalize_email(current) == normalize_email(email))
        || current.contains('@');
    let changes = Changes {
        name: update_name.then(|| contact.name.clone()),
        email: contact.email.clone().filter(|email| {
            normalize_email(email).is_some() && normalize_email(&human.email).is_none()
        }),
        company: contact
            .company
            .clone()
            .filter(|_| human.organization.is_empty()),
    };
    let outcome = if changes == Changes::default() {
        Outcome::UpToDate
    } else {
        Outcome::Enhanced
    };
    (outcome, changes, false)
}

fn person(row: Option<&Value>) -> Option<Person> {
    let row = row?;
    let text = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
    Some(Person {
        name: text("name"),
        email: text("email"),
        organization: text("organization_id"),
    })
}

/// Shipping's `useEventContactEnhancement` mutation for one participant's contact.
pub(super) async fn enhance(
    runtime: &RuntimeHandle,
    session: SessionId,
    human_id: String,
) -> Result<Outcome> {
    runtime
        .submit(move |services| async move {
            let executor = &services.executor;
            let query = |sql: &'static str, params: Vec<Value>| async move {
                executor.execute(sql.into(), params).await.map_err(failure)
            };
            let rows = query(
                "SELECT owner_user_id, event_json FROM sessions WHERE id = ? AND deleted_at IS NULL",
                vec![json!(session)],
            )
            .await?;
            let row = rows.first().ok_or(ServiceError::Conflict)?;
            let user_id = row["owner_user_id"].as_str().unwrap_or_default().to_owned();
            let event: Value =
                serde_json::from_str(row["event_json"].as_str().unwrap_or_default())
                    .unwrap_or(Value::Null);
            if !event.is_object() || user_id.is_empty() {
                return Err(failure("Event unavailable"));
            }
            let attendees = query(super::brief::SESSION_EVENT_SQL, vec![json!(session)]).await?;
            let participants = query(
                "SELECT participant.human_id, participant.source,
                   COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
                   COALESCE(NULLIF(human.email, ''), participant.email) AS email
                 FROM session_participants AS participant
                 LEFT JOIN humans AS human ON human.id = participant.human_id AND human.deleted_at IS NULL
                 WHERE participant.session_id = ? AND participant.deleted_at IS NULL
                 ORDER BY name, email, participant.id",
                vec![json!(session)],
            )
            .await?;
            let humans = query(
                "SELECT id, name, email, organization_id FROM humans
                 WHERE id IN (?, ?) AND deleted_at IS NULL",
                vec![json!(human_id), json!(user_id)],
            )
            .await?;
            let text = |row: &Value, key: &str| row[key].as_str().unwrap_or_default().to_owned();
            let mut candidates: Vec<(String, Candidate)> = Vec::new();
            let mut add = |key_id: &str, candidate: Candidate| {
                let email = normalize_email(&candidate.email);
                let name = clean_name_hint(&candidate.name);
                let key = match (email, key_id.trim()) {
                    (Some(email), _) => format!("email:{email}"),
                    (None, id) if !id.is_empty() => format!("human:{id}"),
                    _ if !name.is_empty() => format!("name:{}", normalize_name(&name)),
                    _ => return,
                };
                match candidates.iter_mut().find(|(existing, _)| *existing == key) {
                    Some((_, existing)) => {
                        if existing.name.is_empty() {
                            existing.name = if name.is_empty() { candidate.name } else { name };
                        }
                        if existing.email.is_empty() {
                            existing.email = candidate.email;
                        }
                        existing.is_current_user |= candidate.is_current_user;
                    }
                    None => candidates.push((
                        key,
                        Candidate {
                            name: if name.is_empty() { candidate.name } else { name },
                            ..candidate
                        },
                    )),
                }
            };
            for row in participants.iter().filter(|row| row["source"] != "excluded") {
                let id = text(row, "human_id");
                add(
                    &id,
                    Candidate {
                        name: text(row, "name"),
                        email: text(row, "email"),
                        is_current_user: id == user_id,
                    },
                );
            }
            let attendees: Vec<Value> = attendees
                .first()
                .and_then(|event| serde_json::from_str(event["participants_json"].as_str()?).ok())
                .unwrap_or_default();
            for attendee in attendees.iter().filter(|attendee| attendee.is_object()) {
                add(
                    "",
                    Candidate {
                        name: text(attendee, "name"),
                        email: text(attendee, "email"),
                        is_current_user: attendee["is_current_user"] == true,
                    },
                );
            }
            let candidates: Vec<Candidate> =
                candidates.into_iter().map(|(_, candidate)| candidate).collect();
            let mut contacts = infer_from_text(
                event["title"].as_str().unwrap_or_default(),
                event["description"].as_str().unwrap_or_default(),
                &candidates,
            );
            contacts.extend(
                candidates
                    .iter()
                    .filter(|candidate| !candidate.is_current_user)
                    .map(|candidate| Contact {
                        name: candidate.name.clone(),
                        email: Some(candidate.email.clone()),
                        company: None,
                    }),
            );
            let contacts = normalize_contacts(contacts, &candidates);
            let human = person(humans.iter().find(|row| row["id"] == json!(human_id)));
            let user = person(humans.iter().find(|row| row["id"] == json!(user_id)));
            let participant = participants
                .iter()
                .find(|row| row["human_id"] == json!(human_id))
                .map(|row| {
                    (
                        text(row, "source"),
                        Person {
                            name: text(row, "name"),
                            email: text(row, "email"),
                            organization: String::new(),
                        },
                    )
                });
            let (outcome, changes, create) = plan(
                &human_id,
                &user_id,
                human.as_ref(),
                user.as_ref(),
                participant
                    .as_ref()
                    .map(|(source, person)| (source.as_str(), person)),
                contacts,
            );
            if create || changes != Changes::default() {
                ops::apply(
                    services.db.pool(),
                    Write::Enhance {
                        id: human_id,
                        owner: user_id,
                        create,
                        name: changes.name,
                        email: changes.email,
                        company: changes.company,
                    },
                )
                .await?;
            }
            Ok(outcome)
        })?
        .receive()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enhancing_a_participant_fills_name_email_and_company_from_the_event() {
        futures::executor::block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
                database: directory.path().join("library.sqlite"),
            })
            .unwrap();
            ready.receive().await.unwrap();
            let sql = |sql: &'static str| {
                let runtime = runtime.clone();
                async move {
                    runtime
                        .submit(move |services| async move {
                            services
                                .executor
                                .execute(sql.into(), vec![])
                                .await
                                .map_err(failure)
                        })
                        .unwrap()
                        .receive()
                        .await
                        .unwrap()
                }
            };
            sql(r#"INSERT INTO sessions(id,title,owner_user_id,event_id,event_json) VALUES ('s','Pricing','me','e','{"title":"Pricing sync between Ana Lima and Jordan"}')"#).await;
            sql(r#"INSERT INTO events(id,title,participants_json) VALUES ('e','Pricing sync','[{"name":"Ana L.","email":"ana.lima@initech.com"},{"name":"Me","email":"me@initech.com","is_current_user":true}]')"#).await;
            sql("INSERT INTO humans(id,owner_user_id,name) VALUES ('ana','me','')").await;
            sql("INSERT INTO session_participants(id,session_id,human_id,owner_user_id,display_name,source) VALUES ('p','s','ana','me','Ana Lima','auto')").await;
            let session = SessionId("s".into());
            // The title names Ana; the attendee list has her work email, which names Initech.
            assert_eq!(
                enhance(&runtime, session.clone(), "ana".into())
                    .await
                    .unwrap(),
                Outcome::Enhanced
            );
            let human = sql("SELECT h.name, h.email, o.name AS company FROM humans h JOIN organizations o ON o.id = h.organization_id WHERE h.id = 'ana'").await;
            assert_eq!(
                (&human[0]["name"], &human[0]["email"], &human[0]["company"]),
                (
                    &json!("Ana Lima"),
                    &json!("ana.lima@initech.com"),
                    &json!("Initech")
                )
            );
            assert_eq!(
                enhance(&runtime, session, "ana".into()).await.unwrap(),
                Outcome::UpToDate
            );
            runtime.shutdown().await.unwrap();
        });
    }
}
