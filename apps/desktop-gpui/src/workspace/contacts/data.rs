//! Contact records and the list, duplicate and related-note rules from shipping's
//! `contacts/queries.ts`, `sidebar/contacts.tsx`, `contacts/index.tsx`, `contacts/details.tsx`
//! and `contacts/related-notes.tsx`.

use std::{cmp::Ordering, collections::HashSet};

use gpui::SharedString;
use serde_json::Value;

pub const LOCAL_OWNER: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Human {
    pub id: SharedString,
    pub created_at: SharedString,
    pub organization_id: SharedString,
    pub name: SharedString,
    pub email: SharedString,
    pub phone: SharedString,
    pub job_title: SharedString,
    pub linkedin: SharedString,
    pub memo: SharedString,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub avatar: Option<SharedString>,
    pub summary: Vec<SharedString>,
    pub additional_emails: Vec<SharedString>,
}

impl Human {
    /// Seed and label for the generated avatar.
    pub fn avatar_name(&self) -> SharedString {
        [&self.name, &self.email, &self.id]
            .into_iter()
            .find(|value| !value.is_empty())
            .cloned()
            .unwrap_or_default()
    }

    pub fn display_name(&self) -> SharedString {
        if !self.name.is_empty() {
            self.name.clone()
        } else if !self.email.is_empty() {
            self.email.clone()
        } else {
            "Unnamed".into()
        }
    }

    fn emails(&self) -> HashSet<String> {
        std::iter::once(&self.email)
            .chain(&self.additional_emails)
            .map(|email| email.trim().to_lowercase())
            .filter(|email| !email.is_empty())
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Organization {
    pub id: SharedString,
    pub created_at: SharedString,
    pub name: SharedString,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub avatar: Option<SharedString>,
    pub team_workspace: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContactsData {
    pub humans: Vec<Human>,
    pub organizations: Vec<Organization>,
    /// Shipping's `useOwnerUserId`: the local library owner when nobody is signed in.
    pub local_owner: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Person(SharedString),
    Organization(SharedString),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOption {
    #[default]
    Alphabetical,
    ReverseAlphabetical,
    Oldest,
    Newest,
}

impl SortOption {
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::Alphabetical, "A-Z"),
        (Self::ReverseAlphabetical, "Z-A"),
        (Self::Oldest, "Oldest"),
        (Self::Newest, "Newest"),
    ];
}

#[derive(Clone, Copy, Debug)]
pub enum Item<'a> {
    Person(&'a Human),
    Organization(&'a Organization),
}

impl Item<'_> {
    pub fn selection(&self) -> Selection {
        match self {
            Self::Person(human) => Selection::Person(human.id.clone()),
            Self::Organization(organization) => Selection::Organization(organization.id.clone()),
        }
    }

    fn pin_order(&self) -> Option<i64> {
        match self {
            Self::Person(human) => human.pin_order,
            Self::Organization(organization) => organization.pin_order,
        }
    }
}

#[derive(Debug, Default)]
pub struct Sections<'a> {
    pub team: Vec<&'a Organization>,
    pub pinned: Vec<Item<'a>>,
    pub others: Vec<Item<'a>>,
}

/// JavaScript's `localeCompare` for names: case-insensitive first, lowercase before uppercase.
fn locale_compare(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| b.cmp(a))
}

fn compare(sort: SortOption, a: (&str, &str), b: (&str, &str)) -> Ordering {
    match sort {
        SortOption::Alphabetical => locale_compare(a.0, b.0),
        SortOption::ReverseAlphabetical => locale_compare(b.0, a.0),
        SortOption::Oldest => a.1.cmp(b.1),
        SortOption::Newest => b.1.cmp(a.1),
    }
}

impl ContactsData {
    pub fn owner(&self, viewer: Option<&str>) -> SharedString {
        viewer
            .map(SharedString::new)
            .unwrap_or_else(|| self.local_owner.clone())
    }

    pub fn human(&self, id: &str) -> Option<&Human> {
        self.humans.iter().find(|human| human.id == id)
    }

    pub fn organization(&self, id: &str) -> Option<&Organization> {
        self.organizations
            .iter()
            .find(|organization| organization.id == id)
    }

    /// The sidebar's self row, team workspaces, pinned items and everything else.
    pub fn sections(&self, owner: &str, search: &str, sort: SortOption) -> Sections<'_> {
        let query = search.trim().to_lowercase();
        let mut humans = self
            .humans
            .iter()
            .filter(|human| human.id != owner)
            .filter(|human| {
                query.is_empty()
                    || [&human.name, &human.email, &human.phone]
                        .iter()
                        .any(|value| value.to_lowercase().contains(&query))
            })
            .collect::<Vec<_>>();
        humans.sort_by(|a, b| compare(sort, (&a.name, &a.created_at), (&b.name, &b.created_at)));
        let mut organizations = self
            .organizations
            .iter()
            .filter(|organization| {
                query.is_empty() || organization.name.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        organizations
            .sort_by(|a, b| compare(sort, (&a.name, &a.created_at), (&b.name, &b.created_at)));
        let (team, others): (Vec<_>, Vec<_>) = organizations
            .into_iter()
            .partition(|organization| organization.team_workspace);
        let mut pinned = humans
            .iter()
            .filter(|human| human.pinned)
            .map(|human| Item::Person(human))
            .chain(
                others
                    .iter()
                    .filter(|organization| organization.pinned)
                    .map(|organization| Item::Organization(organization)),
            )
            .collect::<Vec<_>>();
        pinned.sort_by_key(|item| item.pin_order().unwrap_or(i64::MAX));
        let others = others
            .into_iter()
            .filter(|organization| !organization.pinned)
            .map(Item::Organization)
            .chain(
                humans
                    .into_iter()
                    .filter(|human| !human.pinned)
                    .map(Item::Person),
            )
            .collect();
        Sections {
            team,
            pinned,
            others,
        }
    }

    /// `ContactView`'s fallback when nothing is selected: yourself, then the first person, then
    /// the first organization.
    pub fn effective_selection(
        &self,
        selected: Option<&Selection>,
        owner: &str,
    ) -> Option<Selection> {
        selected.cloned().or_else(|| {
            self.human(owner)
                .or_else(|| self.humans.first())
                .map(|human| Selection::Person(human.id.clone()))
                .or_else(|| {
                    self.organizations
                        .first()
                        .map(|organization| Selection::Organization(organization.id.clone()))
                })
        })
    }

    /// Other people sharing an email address or the exact name with `human`.
    pub fn duplicates(&self, human: &Human, owner: &str) -> Vec<&Human> {
        let emails = human.emails();
        let name = human.name.trim().to_lowercase();
        self.humans
            .iter()
            .filter(|candidate| candidate.id != human.id && candidate.id != owner)
            .filter(|candidate| {
                !candidate.emails().is_disjoint(&emails)
                    || (!name.is_empty() && candidate.name.trim().to_lowercase() == name)
            })
            .collect()
    }

    pub fn members(&self, organization: &str) -> Vec<&Human> {
        self.humans
            .iter()
            .filter(|human| human.organization_id == organization)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedNote {
    pub id: SharedString,
    pub title: SharedString,
    pub created_at: SharedString,
}

fn timestamp(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.timestamp_millis())
        .unwrap_or(0)
}

/// `sortAndFilterRelatedNotes`.
pub fn visible_notes<'a>(
    notes: &'a [RelatedNote],
    search: &str,
    newest: bool,
) -> Vec<&'a RelatedNote> {
    let search = search.trim().to_lowercase();
    let mut visible = notes
        .iter()
        .filter(|note| search.is_empty() || note.title.to_lowercase().contains(&search))
        .collect::<Vec<_>>();
    visible.sort_by(|a, b| {
        let order = timestamp(&a.created_at)
            .cmp(&timestamp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id));
        if newest { order.reverse() } else { order }
    });
    visible
}

/// `toLocaleDateString()` in the en-US locale.
pub fn note_date(value: &str) -> Option<String> {
    let date = chrono::DateTime::parse_from_rfc3339(value)
        .ok()?
        .with_timezone(&chrono::Local);
    Some(date.format("%-m/%-d/%Y").to_string())
}

fn text(row: &Value, key: &str) -> SharedString {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
        .into()
}

fn optional(row: &Value, key: &str) -> Option<SharedString> {
    row.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned().into())
}

fn flag(row: &Value, key: &str) -> bool {
    row.get(key)
        .is_some_and(|value| value == true || value.as_i64().is_some_and(|number| number != 0))
}

fn strings(value: &Value) -> Vec<SharedString> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| text.to_owned().into())
        .collect()
}

/// `parseContactSummary`: only complete, saved summaries are shown.
fn summary(row: &Value) -> Vec<SharedString> {
    let Some(value) = row
        .get("summary")
        .and_then(Value::as_str)
        .and_then(|json| serde_json::from_str::<Value>(json).ok())
    else {
        return Vec::new();
    };
    let facts = strings(&value["facts"]);
    if facts.len() < 3 || !value["sourceHash"].is_string() || !value["generatedAt"].is_string() {
        return Vec::new();
    }
    facts
}

/// Reads the tables `CONTACTS_SQL` reads, so it notifies on every change to them.
pub const CONTACTS_CHANGES_SQL: &str = "SELECT EXISTS(SELECT 1 FROM humans) AS humans,
    EXISTS(SELECT 1 FROM organizations) AS organizations,
    EXISTS(SELECT 1 FROM sessions) AS sessions,
    EXISTS(SELECT 1 FROM chat_groups) AS chat_groups";

pub const CONTACTS_SQL: &str = "SELECT 'human' AS kind, id, created_at, organization_id, name,
    email, phone, job_title, linkedin_username, memo, pinned, pin_order,
    CASE WHEN json_valid(metadata_json) THEN json_extract(metadata_json, '$.avatarDataUrl') END AS avatar,
    CASE WHEN json_valid(metadata_json) THEN json_extract(metadata_json, '$.contactSummary') END AS summary,
    CASE WHEN json_valid(metadata_json) AND json_type(metadata_json, '$.additionalEmails') = 'array'
        THEN json_extract(metadata_json, '$.additionalEmails') END AS additional_emails,
    0 AS team_workspace
  FROM humans WHERE deleted_at IS NULL
  UNION ALL
  SELECT 'organization', id, created_at, '', name, '', '', '', '', memo, pinned, pin_order,
    CASE WHEN json_valid(metadata_json) THEN json_extract(metadata_json, '$.avatarDataUrl') END,
    NULL, NULL,
    CASE WHEN json_valid(metadata_json)
        THEN COALESCE(json_extract(metadata_json, '$.teamWorkspace'), 0) ELSE 0 END
  FROM organizations WHERE deleted_at IS NULL
  UNION ALL
  SELECT 'owner', COALESCE((
    SELECT user_id FROM (
      SELECT owner_user_id AS user_id, updated_at, 0 AS source_priority
      FROM sessions WHERE owner_user_id <> '' AND deleted_at IS NULL
      UNION ALL
      SELECT id, updated_at, 1 FROM humans
      WHERE id = owner_user_id AND id <> '' AND deleted_at IS NULL
      UNION ALL
      SELECT owner_user_id, updated_at, 2 FROM chat_groups
      WHERE owner_user_id <> '' AND deleted_at IS NULL
    ) ORDER BY source_priority, updated_at DESC, user_id LIMIT 1
  ), ''), '', '', '', '', '', '', '', '', 0, NULL, NULL, NULL, NULL, 0";

pub fn decode(rows: &[Value]) -> ContactsData {
    let mut data = ContactsData {
        local_owner: LOCAL_OWNER.into(),
        ..Default::default()
    };
    for row in rows {
        match row["kind"].as_str() {
            Some("human") => data.humans.push(Human {
                id: text(row, "id"),
                created_at: text(row, "created_at"),
                organization_id: text(row, "organization_id"),
                name: text(row, "name"),
                email: text(row, "email"),
                phone: text(row, "phone"),
                job_title: text(row, "job_title"),
                linkedin: text(row, "linkedin_username"),
                memo: text(row, "memo"),
                pinned: flag(row, "pinned"),
                pin_order: row["pin_order"].as_i64(),
                avatar: optional(row, "avatar"),
                summary: summary(row),
                additional_emails: row["additional_emails"]
                    .as_str()
                    .and_then(|json| serde_json::from_str::<Value>(json).ok())
                    .map(|value| strings(&value))
                    .unwrap_or_default(),
            }),
            Some("organization") => data.organizations.push(Organization {
                id: text(row, "id"),
                created_at: text(row, "created_at"),
                name: text(row, "name"),
                pinned: flag(row, "pinned"),
                pin_order: row["pin_order"].as_i64(),
                avatar: optional(row, "avatar"),
                team_workspace: flag(row, "team_workspace"),
            }),
            Some("owner") => {
                let owner = row["id"].as_str().unwrap_or_default().trim();
                if !owner.is_empty() {
                    data.local_owner = owner.to_owned().into();
                }
            }
            _ => {}
        }
    }
    // Shipping's `ORDER BY name, email, id` decides the fallback selection.
    data.humans
        .sort_by(|a, b| (&a.name, &a.email, &a.id).cmp(&(&b.name, &b.email, &b.id)));
    data.organizations
        .sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    data
}

pub const NOTES_SQL: &str = "SELECT sessions.id, sessions.title, sessions.created_at
  FROM sessions
  WHERE sessions.deleted_at IS NULL AND EXISTS (
    SELECT 1 FROM session_participants AS mapping
    WHERE mapping.session_id = sessions.id AND mapping.human_id = ?
      AND mapping.source <> 'excluded' AND mapping.deleted_at IS NULL
  )
  ORDER BY sessions.created_at DESC, sessions.id";

pub fn decode_notes(rows: &[Value]) -> Vec<RelatedNote> {
    rows.iter()
        .map(|row| RelatedNote {
            id: text(row, "id"),
            title: text(row, "title"),
            created_at: text(row, "created_at"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human(id: &str, name: &str, email: &str) -> Human {
        Human {
            id: id.to_owned().into(),
            name: name.to_owned().into(),
            email: email.to_owned().into(),
            ..Default::default()
        }
    }

    fn organization(id: &str, name: &str) -> Organization {
        Organization {
            id: id.to_owned().into(),
            name: name.to_owned().into(),
            ..Default::default()
        }
    }

    fn ids(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|item| match item {
                Item::Person(human) => human.id.to_string(),
                Item::Organization(organization) => organization.id.to_string(),
            })
            .collect()
    }

    #[test]
    fn sidebar_lists_team_workspaces_then_pins_by_order_then_organizations_before_people() {
        let mut pinned_person = human("p-pinned", "Zed", "");
        pinned_person.pinned = true;
        pinned_person.pin_order = Some(2);
        let mut pinned_org = organization("o-pinned", "Acme");
        pinned_org.pinned = true;
        pinned_org.pin_order = Some(1);
        let mut team = organization("o-team", "Team");
        team.team_workspace = true;
        let data = ContactsData {
            humans: vec![
                human("me", "Me", ""),
                human("p-bob", "bob", ""),
                human("p-amy", "Amy", ""),
                pinned_person,
            ],
            organizations: vec![organization("o-beta", "Beta"), pinned_org, team],
            local_owner: "me".into(),
        };
        let sections = data.sections("me", "", SortOption::Alphabetical);
        assert_eq!(sections.team.len(), 1);
        assert_eq!(ids(&sections.pinned), ["o-pinned", "p-pinned"]);
        assert_eq!(ids(&sections.others), ["o-beta", "p-amy", "p-bob"]);

        let sections = data.sections("me", "  BO ", SortOption::ReverseAlphabetical);
        assert!(sections.pinned.is_empty());
        assert_eq!(ids(&sections.others), ["p-bob"]);
    }

    #[test]
    fn selection_falls_back_to_yourself_then_the_first_person_then_an_organization() {
        let mut data = ContactsData {
            humans: vec![human("a", "A", ""), human("me", "Me", "")],
            organizations: vec![organization("o", "O")],
            local_owner: "me".into(),
        };
        let chosen = Selection::Organization("o".into());
        assert_eq!(data.effective_selection(Some(&chosen), "me"), Some(chosen));
        assert_eq!(
            data.effective_selection(None, "me"),
            Some(Selection::Person("me".into()))
        );
        assert_eq!(
            data.effective_selection(None, "nobody"),
            Some(Selection::Person("a".into()))
        );
        data.humans.clear();
        assert_eq!(
            data.effective_selection(None, "me"),
            Some(Selection::Organization("o".into()))
        );
    }

    #[test]
    fn duplicates_match_any_shared_email_or_the_same_name_but_never_yourself() {
        let mut maya = human("maya", "Maya Chen", "maya@work.example");
        maya.additional_emails = vec!["M@Home.example".into()];
        let data = ContactsData {
            humans: vec![
                maya.clone(),
                human("same-name", " maya chen ", ""),
                human("shared-email", "Other", "m@home.example"),
                human("me", "Maya Chen", ""),
                human("unrelated", "Sam", "sam@example.com"),
            ],
            ..Default::default()
        };
        let found = data
            .duplicates(&maya, "me")
            .into_iter()
            .map(|human| human.id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(found, ["same-name", "shared-email"]);
        assert!(data.duplicates(&human("x", "", ""), "me").is_empty());
    }

    #[test]
    fn related_notes_filter_by_title_and_sort_by_date_then_id() {
        let note = |id: &str, title: &str, at: &str| RelatedNote {
            id: id.to_owned().into(),
            title: title.to_owned().into(),
            created_at: at.to_owned().into(),
        };
        let notes = [
            note("b", "Pricing", "2026-01-02T00:00:00Z"),
            note("a", "Hiring", "2026-01-02T00:00:00Z"),
            note("c", "Offsite", "2026-03-01T00:00:00Z"),
        ];
        let order = |visible: Vec<&RelatedNote>| {
            visible
                .into_iter()
                .map(|note| note.id.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(visible_notes(&notes, "", true)), ["c", "b", "a"]);
        assert_eq!(order(visible_notes(&notes, "", false)), ["a", "b", "c"]);
        assert_eq!(order(visible_notes(&notes, " pRICing", true)), ["b"]);
    }

    #[test]
    fn decoding_keeps_only_complete_summaries_and_resolves_the_local_owner() {
        let rows = [
            serde_json::json!({"kind":"human","id":"h","name":"N","pinned":1,"pin_order":3,
                "additional_emails":"[\"x@y.z\",\"\",4]",
                "summary":"{\"facts\":[\"a\",\"b\",\"c\"],\"sourceHash\":\"s\",\"generatedAt\":\"g\"}"}),
            serde_json::json!({"kind":"human","id":"g","name":"M",
                "summary":"{\"facts\":[\"a\",\"b\"],\"sourceHash\":\"s\",\"generatedAt\":\"g\"}"}),
            serde_json::json!({"kind":"organization","id":"o","team_workspace":1}),
            serde_json::json!({"kind":"owner","id":" me "}),
        ];
        let data = decode(&rows);
        let h = data.human("h").unwrap();
        assert!(h.pinned && h.pin_order == Some(3));
        assert_eq!(h.summary.len(), 3);
        assert_eq!(h.additional_emails, vec![SharedString::from("x@y.z")]);
        assert!(data.human("g").unwrap().summary.is_empty());
        assert!(data.organization("o").unwrap().team_workspace);
        assert_eq!(data.local_owner, "me");
        assert_eq!(decode(&[]).local_owner, LOCAL_OWNER);
    }
}
