use anlg_db_app::{ParticipantHumanRow, ParticipantMappingRow, SessionSyncRow};

use super::connection_sync::IncomingEventParticipant;
use super::identity::{NameSource, derive_contact_identity, is_email_placeholder_name};
use indexmap::IndexMap;

pub(crate) struct ParticipantSyncSnapshot<'a> {
    pub sessions: Vec<SessionSyncRow>,
    pub humans: &'a [ParticipantHumanRow],
    pub mappings: &'a [ParticipantMappingRow],
}

#[derive(Debug)]
pub(crate) struct HumanToCreate {
    pub id: String,
    pub owner_user_id: String,
    pub name: String,
    pub email: String,
    pub company_name: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct HumanToEnrich {
    pub id: String,
    pub owner_user_id: String,
    pub name: Option<String>,
    pub company_name: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ParticipantMappingToAdd {
    pub session_id: String,
    pub human_id: String,
    pub email: String,
}

#[derive(Debug, Default)]
pub(crate) struct ParticipantsSyncOutput {
    pub to_delete: Vec<String>,
    pub to_add: Vec<ParticipantMappingToAdd>,
    pub humans_to_create: Vec<HumanToCreate>,
    pub humans_to_enrich: Vec<HumanToEnrich>,
}

pub(crate) fn sync_session_participants(
    incoming_participants: &IndexMap<String, Vec<IncomingEventParticipant>>,
    snapshot: ParticipantSyncSnapshot<'_>,
) -> ParticipantsSyncOutput {
    let mut output = ParticipantsSyncOutput::default();

    let mut humans_by_email: IndexMap<String, String> = IndexMap::new();
    let mut humans_by_id: IndexMap<String, &ParticipantHumanRow> = IndexMap::new();
    for human in snapshot.humans {
        humans_by_id.insert(human.id.clone(), human);
        let email = human.email.trim().to_lowercase();
        if !email.is_empty() && !humans_by_email.contains_key(&email) {
            humans_by_email.insert(email, human.id.clone());
        }
    }

    let mut mappings_by_session: IndexMap<String, IndexMap<String, &ParticipantMappingRow>> =
        IndexMap::new();
    for mapping in snapshot.mappings {
        if !mappings_by_session.contains_key(&mapping.session_id) {
            mappings_by_session.insert(mapping.session_id.clone(), IndexMap::new());
        }
        let session_mappings = mappings_by_session.get_mut(&mapping.session_id).unwrap();
        if !session_mappings.contains_key(&mapping.human_id) {
            session_mappings.insert(mapping.human_id.clone(), mapping);
        }
    }

    let mut humans_to_create: IndexMap<String, HumanToCreate> = IndexMap::new();
    let mut humans_to_enrich: IndexMap<String, HumanToEnrich> = IndexMap::new();

    for session in &snapshot.sessions {
        let Some(event_participants) = incoming_participants.get(&session.tracking_id) else {
            continue;
        };

        let empty: IndexMap<String, &ParticipantMappingRow> = IndexMap::new();
        let existing_mappings = mappings_by_session.get(&session.id).unwrap_or(&empty);
        let changes = compute_session_participant_changes(
            &session.id,
            &session.owner_user_id,
            event_participants,
            &mut humans_by_email,
            &humans_by_id,
            &mut humans_to_create,
            &mut humans_to_enrich,
            existing_mappings,
        );
        output.to_delete.extend(changes.0);
        output.to_add.extend(changes.1);
    }

    output.humans_to_create = humans_to_create
        .iter()
        .map(|(_, human)| HumanToCreate {
            id: human.id.clone(),
            owner_user_id: human.owner_user_id.clone(),
            name: human.name.clone(),
            email: human.email.clone(),
            company_name: human.company_name.clone(),
        })
        .collect();
    output.humans_to_enrich = humans_to_enrich
        .iter()
        .map(|(_, human)| HumanToEnrich {
            id: human.id.clone(),
            owner_user_id: human.owner_user_id.clone(),
            name: human.name.clone(),
            company_name: human.company_name.clone(),
        })
        .collect();
    output
}

#[allow(clippy::too_many_arguments)]
fn compute_session_participant_changes<'a>(
    session_id: &str,
    owner_user_id: &str,
    event_participants: &[IncomingEventParticipant],
    humans_by_email: &mut IndexMap<String, String>,
    humans_by_id: &IndexMap<String, &'a ParticipantHumanRow>,
    humans_to_create: &mut IndexMap<String, HumanToCreate>,
    humans_to_enrich: &mut IndexMap<String, HumanToEnrich>,
    existing_mappings: &IndexMap<String, &'a ParticipantMappingRow>,
) -> (Vec<String>, Vec<ParticipantMappingToAdd>) {
    let mut event_humans: IndexMap<String, (String, String)> = IndexMap::new();

    for participant in event_participants {
        let email = participant
            .email
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if email.is_empty() {
            continue;
        }

        let email_key = email.to_lowercase();
        let identity = derive_contact_identity(participant.name.as_deref(), &email);
        let human_id = match humans_by_email.get(&email_key) {
            None => {
                let new_id = uuid::Uuid::new_v4().to_string();
                humans_by_email.insert(email_key.clone(), new_id.clone());
                humans_to_create.insert(
                    email_key.clone(),
                    HumanToCreate {
                        id: new_id.clone(),
                        owner_user_id: owner_user_id.to_string(),
                        name: identity.name.clone(),
                        email: email.clone(),
                        company_name: identity.company_name.clone(),
                    },
                );
                new_id
            }
            Some(human_id) => {
                let human_id = human_id.clone();
                if humans_to_create.contains_key(&email_key) {
                    if identity.name_source == NameSource::Provider
                        && let Some(pending) = humans_to_create.get_mut(&email_key)
                    {
                        pending.name = identity.name.clone();
                    }
                } else if human_id != owner_user_id
                    && let Some(existing) = humans_by_id.get(&human_id)
                    && let Some(enrichment) = plan_human_enrichment(
                        existing,
                        &identity,
                        &email,
                        owner_user_id,
                        humans_to_enrich.get(&human_id),
                    )
                {
                    humans_to_enrich.insert(human_id.clone(), enrichment);
                }
                human_id
            }
        };
        event_humans.insert(human_id.clone(), (human_id, email));
    }

    let mut to_add: Vec<ParticipantMappingToAdd> = Vec::new();
    let mut to_delete: Vec<String> = Vec::new();
    for (_, (human_id, email)) in event_humans.iter() {
        if !existing_mappings.contains_key(human_id) {
            to_add.push(ParticipantMappingToAdd {
                session_id: session_id.to_string(),
                human_id: human_id.clone(),
                email: email.clone(),
            });
        }
    }

    for (human_id, mapping) in existing_mappings.iter() {
        if mapping.source == "auto" && !event_humans.contains_key(human_id) {
            to_delete.push(mapping.id.clone());
        }
    }

    (to_delete, to_add)
}

fn plan_human_enrichment(
    existing: &ParticipantHumanRow,
    identity: &super::identity::DerivedContactIdentity,
    email: &str,
    owner_user_id: &str,
    pending: Option<&HumanToEnrich>,
) -> Option<HumanToEnrich> {
    let mut enrichment = HumanToEnrich {
        id: existing.id.clone(),
        owner_user_id: owner_user_id.to_string(),
        name: None,
        company_name: None,
    };

    let current_name = existing.name.trim();
    let name_needs_fill = is_email_placeholder_name(current_name);
    let pending_name = pending
        .and_then(|pending| pending.name.clone())
        .filter(|name| !name.is_empty());
    if pending_name.is_some() && identity.name_source != NameSource::Provider {
        enrichment.name = pending_name;
    } else if name_needs_fill && identity.name != email && identity.name != current_name {
        enrichment.name = Some(identity.name.clone());
    }
    if existing.organization_id.is_empty() {
        enrichment.company_name = identity.company_name.clone();
    }

    if enrichment.name.is_some() || enrichment.company_name.is_some() {
        Some(enrichment)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_app::{ParticipantHumanRow, ParticipantMappingRow, SessionSyncRow};

    fn participant(email: &str, name: Option<&str>) -> IncomingEventParticipant {
        IncomingEventParticipant {
            name: name.map(str::to_string),
            email: Some(email.to_string()),
            is_organizer: false,
            is_current_user: false,
        }
    }

    fn session(id: &str, tracking_id: &str) -> SessionSyncRow {
        SessionSyncRow {
            id: id.to_string(),
            owner_user_id: "user-1".to_string(),
            event_json: "{}".to_string(),
            tracking_id: tracking_id.to_string(),
            calendar_id: String::new(),
        }
    }

    fn human(id: &str, email: &str, name: &str, organization_id: &str) -> ParticipantHumanRow {
        ParticipantHumanRow {
            id: id.to_string(),
            email: email.to_string(),
            name: name.to_string(),
            organization_id: organization_id.to_string(),
        }
    }

    fn mapping(id: &str, session_id: &str, human_id: &str, source: &str) -> ParticipantMappingRow {
        ParticipantMappingRow {
            id: id.to_string(),
            session_id: session_id.to_string(),
            human_id: human_id.to_string(),
            source: source.to_string(),
        }
    }

    fn incoming(
        entries: Vec<(&str, Vec<IncomingEventParticipant>)>,
    ) -> IndexMap<String, Vec<IncomingEventParticipant>> {
        let mut map = IndexMap::new();
        for (key, value) in entries {
            map.insert(key.to_string(), value);
        }
        map
    }

    fn snapshot<'a>(
        sessions: Vec<SessionSyncRow>,
        humans: &'a [ParticipantHumanRow],
        mappings: &'a [ParticipantMappingRow],
    ) -> ParticipantSyncSnapshot<'a> {
        ParticipantSyncSnapshot {
            sessions,
            humans,
            mappings,
        }
    }

    #[test]
    fn updates_participants_on_every_note_attached_to_a_reconciled_occurrence() {
        let result = sync_session_participants(
            &incoming(vec![(
                "tracking-1",
                vec![participant("guest@example.com", None)],
            )]),
            snapshot(
                vec![
                    session("session-1", "tracking-1"),
                    session("session-2", "tracking-1"),
                ],
                &[],
                &[],
            ),
        );
        assert_eq!(
            result
                .to_add
                .iter()
                .map(|m| m.session_id.as_str())
                .collect::<Vec<_>>(),
            ["session-1", "session-2"]
        );
        assert_eq!(result.humans_to_create.len(), 1);
        assert_eq!(result.to_add[0].human_id, result.to_add[1].human_id);
    }

    #[test]
    fn creates_a_human_when_the_participant_email_is_new() {
        let result = sync_session_participants(
            &incoming(vec![(
                "tracking-1",
                vec![participant("new@example.com", Some("New Person"))],
            )]),
            snapshot(vec![session("session-1", "tracking-1")], &[], &[]),
        );
        assert_eq!(result.humans_to_create.len(), 1);
        let human = &result.humans_to_create[0];
        assert_eq!(human.owner_user_id, "user-1");
        assert_eq!(human.email, "new@example.com");
        assert_eq!(human.name, "New Person");
        assert_eq!(human.company_name.as_deref(), Some("Example"));
        assert!(result.humans_to_enrich.is_empty());
        assert_eq!(result.to_add.len(), 1);
        assert_eq!(result.to_add[0].session_id, "session-1");
        assert_eq!(result.to_add[0].human_id, human.id);
        assert_eq!(result.to_add[0].email, "new@example.com");
    }

    #[test]
    fn uses_an_existing_human_when_email_matches_case_insensitively() {
        let humans = [human(
            "human-1",
            "existing@example.com",
            "Existing",
            "org-1",
        )];
        let result = sync_session_participants(
            &incoming(vec![(
                "tracking-1",
                vec![participant("Existing@Example.com", Some("Existing"))],
            )]),
            snapshot(vec![session("session-1", "tracking-1")], &humans, &[]),
        );
        assert!(result.humans_to_create.is_empty());
        assert_eq!(result.to_add[0].human_id, "human-1");
    }

    #[test]
    fn a_removed_auto_participant_is_deleted_but_an_excluded_one_is_kept() {
        let humans = [human("human-1", "removed@example.com", "", "")];
        let auto = [mapping("mapping-1", "session-1", "human-1", "auto")];
        let result = sync_session_participants(
            &incoming(vec![("tracking-1", vec![])]),
            snapshot(vec![session("session-1", "tracking-1")], &humans, &auto),
        );
        assert_eq!(result.to_delete, ["mapping-1"]);

        let excluded = [mapping("mapping-1", "session-1", "human-1", "excluded")];
        let result = sync_session_participants(
            &incoming(vec![("tracking-1", vec![])]),
            snapshot(vec![session("session-1", "tracking-1")], &humans, &excluded),
        );
        assert!(result.to_delete.is_empty());
    }

    #[test]
    fn enriches_existing_humans_whose_name_is_missing_or_an_email() {
        let humans = [
            human("human-a", "alice@acme.com", "alice@acme.com", ""),
            human("human-b", "bob@acme.com", "", "org-1"),
            human("human-c", "done@acme.com", "Done Person", "org-1"),
        ];
        let result = sync_session_participants(
            &incoming(vec![(
                "tracking-1",
                vec![
                    participant("alice@acme.com", None),
                    participant("bob@acme.com", Some("Bob Builder")),
                    participant("done@acme.com", None),
                ],
            )]),
            snapshot(vec![session("session-1", "tracking-1")], &humans, &[]),
        );
        assert!(result.humans_to_create.is_empty());
        assert_eq!(result.humans_to_enrich.len(), 2);
        assert_eq!(result.humans_to_enrich[0].id, "human-a");
        assert_eq!(result.humans_to_enrich[0].name.as_deref(), Some("Alice"));
        assert_eq!(
            result.humans_to_enrich[0].company_name.as_deref(),
            Some("Acme")
        );
        assert_eq!(result.humans_to_enrich[1].id, "human-b");
        assert_eq!(
            result.humans_to_enrich[1].name.as_deref(),
            Some("Bob Builder")
        );
        assert_eq!(result.humans_to_enrich[1].company_name, None);
    }

    #[test]
    fn never_enriches_the_session_owner() {
        let humans = [human("user-1", "me@acme.com", "", "")];
        let result = sync_session_participants(
            &incoming(vec![("tracking-1", vec![participant("me@acme.com", None)])]),
            snapshot(vec![session("session-1", "tracking-1")], &humans, &[]),
        );
        assert!(result.humans_to_enrich.is_empty());
    }

    #[test]
    fn prefers_a_provider_display_name_over_an_email_derived_one_across_sessions() {
        let humans = [human("human-a", "alice.smith@acme.com", "", "")];
        let result = sync_session_participants(
            &incoming(vec![
                (
                    "tracking-1",
                    vec![participant("alice.smith@acme.com", None)],
                ),
                (
                    "tracking-2",
                    vec![participant("alice.smith@acme.com", Some("Dr. Alice Smith"))],
                ),
            ]),
            snapshot(
                vec![
                    session("session-1", "tracking-1"),
                    session("session-2", "tracking-2"),
                ],
                &humans,
                &[],
            ),
        );
        assert_eq!(result.humans_to_enrich.len(), 1);
        assert_eq!(result.humans_to_enrich[0].id, "human-a");
        assert_eq!(
            result.humans_to_enrich[0].name.as_deref(),
            Some("Dr. Alice Smith")
        );
        assert_eq!(
            result.humans_to_enrich[0].company_name.as_deref(),
            Some("Acme")
        );
    }

    #[test]
    fn upgrades_a_pending_new_humans_name_when_a_later_event_provides_one() {
        let result = sync_session_participants(
            &incoming(vec![
                (
                    "tracking-1",
                    vec![participant("alice.smith@acme.com", None)],
                ),
                (
                    "tracking-2",
                    vec![participant("alice.smith@acme.com", Some("Dr. Alice Smith"))],
                ),
            ]),
            snapshot(
                vec![
                    session("session-1", "tracking-1"),
                    session("session-2", "tracking-2"),
                ],
                &[],
                &[],
            ),
        );
        assert_eq!(result.humans_to_create.len(), 1);
        let human = &result.humans_to_create[0];
        assert_eq!(human.owner_user_id, "user-1");
        assert_eq!(human.email, "alice.smith@acme.com");
        assert_eq!(human.name, "Dr. Alice Smith");
        assert_eq!(human.company_name.as_deref(), Some("Acme"));
    }
}
