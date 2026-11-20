//! Share state the native surfaces render, parsed from the same RPC rows shipping reads
//! (`session-sharing/client-contract.ts`), and the invite-recipient rules of
//! `session-sharing/invite-recipients.tsx`.

use std::{collections::HashMap, sync::Arc};

use desktop_runtime::SessionId;
use gpui::{App, Global};
use serde_json::Value;

/// Ordered from least to most access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    Viewer,
    Commenter,
    Editor,
}

impl Capability {
    pub const ALL: [Self; 3] = [Self::Viewer, Self::Commenter, Self::Editor];

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "viewer" => Some(Self::Viewer),
            "commenter" => Some(Self::Commenter),
            "editor" => Some(Self::Editor),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Commenter => "commenter",
            Self::Editor => "editor",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Viewer => "Can view",
            Self::Commenter => "Can comment",
            Self::Editor => "Can edit",
        }
    }

    pub fn rank(self) -> u8 {
        self as u8 + 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Grant,
    Invitation,
    Request,
}

/// One row of `list_session_share_access`.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessEntry {
    pub kind: EntryKind,
    pub id: Arc<str>,
    pub user_id: Option<Arc<str>>,
    pub email: Option<Arc<str>>,
    pub capability: Capability,
}

impl AccessEntry {
    pub fn parse(row: &Value) -> Option<Self> {
        let kind = match row["entry_type"].as_str()? {
            "grant" => EntryKind::Grant,
            "invitation" => EntryKind::Invitation,
            "request" => EntryKind::Request,
            _ => return None,
        };
        let email = row["user_email"].as_str().map(Arc::from);
        if kind == EntryKind::Invitation && email.is_none() {
            return None;
        }
        Some(Self {
            kind,
            id: row["entry_id"].as_str().filter(|id| !id.is_empty())?.into(),
            user_id: row["user_id"].as_str().map(Arc::from),
            email,
            capability: Capability::parse(row["capability"].as_str()?)?,
        })
    }

    /// `AccessEntryRow`'s name line.
    pub fn label(&self, contact: Option<&str>) -> String {
        contact
            .filter(|name| !name.is_empty())
            .or(self.email.as_deref())
            .unwrap_or("Anarlog user")
            .to_owned()
    }

    /// `AccessEntryRow`'s second line.
    pub fn detail(&self, contact: Option<&str>) -> String {
        match (contact.filter(|name| !name.is_empty()), &self.email) {
            (Some(_), Some(email)) => email.to_string(),
            _ => match self.kind {
                EntryKind::Grant => "Anarlog member".into(),
                EntryKind::Invitation => "Invitation pending".into(),
                EntryKind::Request => {
                    format!("Requested {}", self.capability.label().to_lowercase())
                }
            },
        }
    }
}

/// A shared workspace the account can share into (`useAvailableShareWorkspaces`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareWorkspace {
    pub id: Arc<str>,
    pub name: Arc<str>,
}

/// The general-access select's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneralAccess {
    Restricted,
    Workspace(Arc<str>),
    Link,
    Public,
}

impl GeneralAccess {
    pub fn parse(scope: &str, workspace: Option<&str>) -> Option<Self> {
        match (scope, workspace) {
            ("restricted", None) => Some(Self::Restricted),
            ("workspace", Some(id)) => Some(Self::Workspace(id.into())),
            ("link", None) => Some(Self::Link),
            ("public", None) => Some(Self::Public),
            _ => None,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Restricted => "sharing/LockKeyIcon.svg",
            Self::Workspace(_) => "sharing/Building03Icon.svg",
            Self::Link | Self::Public => "sharing/GlobeIcon.svg",
        }
    }

    pub fn label(&self, workspaces: &[ShareWorkspace]) -> String {
        match self {
            Self::Restricted => "Only people invited".into(),
            Self::Workspace(id) => workspaces
                .iter()
                .find(|workspace| &workspace.id == id)
                .map(|workspace| format!("Everyone in {}", workspace.name))
                .unwrap_or_default(),
            Self::Link => "Anyone with the link".into(),
            Self::Public => "Public on the web".into(),
        }
    }
}

/// `default_meeting_share_access`: what the draft panel's select starts on.
pub fn default_general_access(setting: &str, workspaces: &[ShareWorkspace]) -> GeneralAccess {
    match workspaces.first() {
        Some(workspace) if setting == "workspace" => GeneralAccess::Workspace(workspace.id.clone()),
        _ => GeneralAccess::Restricted,
    }
}

/// A note's published share, as its owner manages it.
#[derive(Clone, Debug, PartialEq)]
pub struct ManagedShare {
    pub share_id: Arc<str>,
    pub general: GeneralAccess,
    pub has_active_link: bool,
    pub access: Arc<[AccessEntry]>,
    pub conflict: bool,
}

/// The share state last loaded for each note, so reopening the popover shows it immediately.
#[derive(Default)]
pub struct Shares(HashMap<SessionId, ManagedShare>);

impl Global for Shares {}

pub fn managed(session: &SessionId, cx: &App) -> Option<ManagedShare> {
    cx.try_global::<Shares>()
        .and_then(|shares| shares.0.get(session).cloned())
}

pub fn remember(session: SessionId, share: Option<ManagedShare>, cx: &mut App) {
    let shares = cx.default_global::<Shares>();
    match share {
        Some(share) => shares.0.insert(session, share),
        None => shares.0.remove(&session),
    };
}

/// `isInviteEmail`: `^[^\s@]+@[^\s@]+\.[^\s@]+$`, at most 320 characters, no control characters.
pub fn is_invite_email(value: &str) -> bool {
    let value = value.trim();
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    let plain = |part: &str| {
        !part.is_empty() && !part.contains('@') && !part.chars().any(char::is_whitespace)
    };
    value.chars().count() <= 320
        && !value.chars().any(|c| c.is_control())
        && plain(local)
        && plain(domain)
        && domain
            .char_indices()
            .any(|(index, c)| c == '.' && index > 0 && index + 1 < domain.len())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipient {
    pub email: Arc<str>,
    pub name: Arc<str>,
}

/// `useShareInvite`: meeting participants are suggested until removed, plus people the user
/// added. Recipients are derived each time so participants that load later still appear.
#[derive(Clone, Debug, Default)]
pub struct Invite {
    pub participants: Vec<Recipient>,
    /// The owner's and already-invited emails.
    pub excluded: Vec<String>,
    added: Vec<Recipient>,
    dismissed: Vec<String>,
}

impl Invite {
    pub fn recipients(&self) -> Vec<Recipient> {
        let mut taken = self.dismissed.clone();
        let mut recipients = Vec::new();
        for candidate in self.participants.iter().chain(&self.added) {
            let key = candidate.email.trim().to_lowercase();
            if !is_invite_email(&candidate.email) || self.is_excluded(&key) || taken.contains(&key)
            {
                continue;
            }
            taken.push(key);
            recipients.push(Recipient {
                email: candidate.email.trim().into(),
                name: candidate.name.trim().into(),
            });
        }
        recipients
    }

    fn is_excluded(&self, key: &str) -> bool {
        self.excluded
            .iter()
            .any(|email| email.trim().to_lowercase() == key)
    }

    pub fn is_selectable(&self, email: &str) -> bool {
        let key = email.trim().to_lowercase();
        is_invite_email(email)
            && !self.is_excluded(&key)
            && !self
                .recipients()
                .iter()
                .any(|recipient| recipient.email.to_lowercase() == key)
    }

    /// The emails the Invite button sends: every recipient plus a valid typed address.
    pub fn emails(&self, typed: &str) -> Vec<String> {
        let typed = typed.trim();
        let mut emails: Vec<String> = self
            .recipients()
            .iter()
            .map(|recipient| recipient.email.to_string())
            .collect();
        if self.is_selectable(typed) {
            emails.push(typed.into());
        }
        emails
    }

    pub fn can_submit(&self, typed: &str) -> bool {
        let typed = typed.trim();
        !self.emails(typed).is_empty() && (typed.is_empty() || is_invite_email(typed))
    }

    pub fn add(&mut self, recipient: Recipient) {
        let key = recipient.email.trim().to_lowercase();
        self.dismissed.retain(|email| *email != key);
        if !self
            .added
            .iter()
            .any(|entry| entry.email.to_lowercase() == key)
        {
            self.added.push(Recipient {
                email: recipient.email.trim().into(),
                name: recipient.name.trim().into(),
            });
        }
    }

    /// Enter in the field: adds a valid typed address. Returns whether to clear the field.
    pub fn commit(&mut self, typed: &str) -> bool {
        let typed = typed.trim();
        if !is_invite_email(typed) {
            return false;
        }
        if self.is_selectable(typed) {
            self.add(Recipient {
                email: typed.into(),
                name: "".into(),
            });
        }
        true
    }

    pub fn remove(&mut self, email: &str) {
        let key = email.trim().to_lowercase();
        self.added.retain(|entry| entry.email.to_lowercase() != key);
        if !self.dismissed.contains(&key) {
            self.dismissed.push(key);
        }
    }

    pub fn restore(&mut self, email: &str) {
        let key = email.trim().to_lowercase();
        self.dismissed.retain(|dismissed| *dismissed != key);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn person(email: &str, name: &str) -> Recipient {
        Recipient {
            email: email.into(),
            name: name.into(),
        }
    }

    #[test]
    fn invite_emails_follow_the_shipping_pattern() {
        for valid in [
            "a@b.co",
            " maya@northwind.example ",
            "x.y+z@sub.example.org",
        ] {
            assert!(is_invite_email(valid), "{valid}");
        }
        for invalid in [
            "",
            "maya",
            "maya@",
            "@northwind.example",
            "maya@northwind",
            "maya@.example",
            "maya@example.",
            "ma ya@example.com",
            "a@b@c.com",
            "a\u{7}@b.com",
        ] {
            assert!(!is_invite_email(invalid), "{invalid}");
        }
        assert!(!is_invite_email(&format!("{}@b.com", "a".repeat(320))));
    }

    #[test]
    fn invite_suggests_participants_until_removed_and_skips_owner_and_invited() {
        let mut invite = Invite {
            participants: vec![
                person("maya@northwind.example", "Maya Chen"),
                person("Owner@Lumen.example", "Alex"),
                person("priya@northwind.example", "Priya Patel"),
                person("MAYA@northwind.example", "Maya again"),
                person("not-an-email", "Nobody"),
            ],
            excluded: vec![
                "owner@lumen.example".into(),
                "priya@northwind.example".into(),
            ],
            ..Invite::default()
        };
        assert_eq!(
            invite.recipients(),
            [person("maya@northwind.example", "Maya Chen")]
        );
        assert_eq!(
            invite.emails("jordan@contoso.example"),
            ["maya@northwind.example", "jordan@contoso.example"]
        );
        // A typed address that is already a recipient, the owner or invited is not doubled.
        assert_eq!(invite.emails("MAYA@northwind.example").len(), 1);
        assert_eq!(invite.emails("owner@lumen.example").len(), 1);
        assert!(invite.can_submit(""));
        assert!(!invite.can_submit("jordan@"));

        invite.remove("Maya@Northwind.example");
        assert!(invite.recipients().is_empty());
        assert!(!invite.can_submit(""));
        assert!(invite.commit("jordan@contoso.example"));
        assert!(!invite.commit("jordan"));
        assert_eq!(invite.emails(""), ["jordan@contoso.example"]);

        // Revoking someone's access suggests them again.
        invite.restore("maya@northwind.example");
        assert_eq!(invite.recipients().len(), 2);
        invite.add(person("maya@northwind.example", "Maya"));
        assert_eq!(invite.recipients().len(), 2);
    }

    #[test]
    fn access_rows_parse_and_describe_like_shipping() {
        let grant = AccessEntry::parse(&json!({"entry_type":"grant","entry_id":"g","user_id":"u","user_email":null,"capability":"commenter"})).unwrap();
        assert_eq!(grant.label(None), "Anarlog user");
        assert_eq!(grant.detail(None), "Anarlog member");
        let invitation = AccessEntry::parse(&json!({"entry_type":"invitation","entry_id":"i","user_id":null,"user_email":"maya@northwind.example","capability":"viewer"})).unwrap();
        assert_eq!(invitation.label(Some("Maya Chen")), "Maya Chen");
        assert_eq!(
            invitation.detail(Some("Maya Chen")),
            "maya@northwind.example"
        );
        assert_eq!(invitation.detail(None), "Invitation pending");
        let request = AccessEntry::parse(&json!({"entry_type":"request","entry_id":"r","user_id":"u2","user_email":"jo@contoso.example","capability":"editor"})).unwrap();
        assert_eq!(request.detail(None), "Requested can edit");
        assert!(AccessEntry::parse(&json!({"entry_type":"invitation","entry_id":"i","user_email":null,"capability":"viewer"})).is_none());
        assert!(
            AccessEntry::parse(&json!({"entry_type":"grant","entry_id":"g","capability":"owner"}))
                .is_none()
        );
        assert!(Capability::Editor.rank() > Capability::Commenter.rank());
    }

    #[test]
    fn general_access_requires_a_workspace_only_for_workspace_scope() {
        let workspaces = [ShareWorkspace {
            id: "w".into(),
            name: "Lumen".into(),
        }];
        assert_eq!(
            GeneralAccess::parse("workspace", Some("w"))
                .unwrap()
                .label(&workspaces),
            "Everyone in Lumen"
        );
        assert!(GeneralAccess::parse("workspace", None).is_none());
        assert!(GeneralAccess::parse("restricted", Some("w")).is_none());
        assert_eq!(
            default_general_access("workspace", &workspaces),
            GeneralAccess::Workspace("w".into())
        );
        assert_eq!(
            default_general_access("workspace", &[]),
            GeneralAccess::Restricted
        );
        assert_eq!(
            default_general_access("participants", &workspaces),
            GeneralAccess::Restricted
        );
    }
}
