use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{Envelope, Error, RecoveryKey, Result, WorkspaceKey};

const MEMBER_IDENTITY_KDF_SALT: &[u8] = b"anarlog-e2ee-member-identity-v1";
const MEMBER_IDENTITY_INFO: &[u8] = b"x25519";
const GRANT_KDF_SALT: &[u8] = b"anarlog-e2ee-workspace-key-grant-kdf-v1";
const GRANT_AAD_DOMAIN: &[u8] = b"anarlog-e2ee-workspace-key-grant-v1";

/// Account-level X25519 keypair derived from the recovery key.
///
/// Every device holding the recovery key re-derives the same secret, so a grant
/// sealed once reaches all of that member's devices without the issuer ever
/// learning the recipient's recovery key.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct MemberIdentityKey([u8; 32]);

impl MemberIdentityKey {
    pub fn public_key(&self) -> String {
        let secret = x25519_dalek::StaticSecret::from(self.0);
        URL_SAFE_NO_PAD.encode(x25519_dalek::PublicKey::from(&secret).as_bytes())
    }

    pub fn open_workspace_key(
        &self,
        workspace_id: &str,
        member_user_id: &str,
        grant: &WorkspaceKeyGrant,
    ) -> Result<WorkspaceKey> {
        let ephemeral_public = decode_grant_bytes(&grant.ephemeral_public_key)?;
        let nonce: [u8; 24] = URL_SAFE_NO_PAD
            .decode(&grant.nonce)
            .map_err(|_| Error::InvalidWorkspaceKeyGrant)?
            .try_into()
            .map_err(|_| Error::InvalidWorkspaceKeyGrant)?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(&grant.ciphertext)
            .map_err(|_| Error::InvalidWorkspaceKeyGrant)?;
        let shared = x25519_dalek::StaticSecret::from(self.0)
            .diffie_hellman(&x25519_dalek::PublicKey::from(ephemeral_public));
        if !shared.was_contributory() {
            return Err(Error::InvalidWorkspaceKeyGrant);
        }
        let cipher = grant_cipher(
            shared.as_bytes(),
            workspace_id,
            member_user_id,
            &grant.key_id,
        )?;
        let plaintext = cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &grant_aad(workspace_id, member_user_id, &grant.key_id),
                },
            )
            .map_err(|_| Error::AuthenticationFailed)?;
        let mut bytes: [u8; 32] = Zeroizing::new(plaintext)
            .as_slice()
            .try_into()
            .map_err(|_| Error::InvalidWorkspaceKeyGrant)?;
        let key = WorkspaceKey::from_bytes(bytes);
        bytes.zeroize();
        // A grant whose label disagrees with its contents is never usable: the
        // keyring indexes by key_id, so accepting it would silently shadow the
        // real key for that id.
        if key.key_id() != grant.key_id {
            return Err(Error::InvalidWorkspaceKeyGrant);
        }
        Ok(key)
    }
}

/// A shared workspace key sealed to one member's identity key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceKeyGrant {
    pub key_id: String,
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

pub fn seal_workspace_key_for_member(
    key: &WorkspaceKey,
    recipient_public_key: &str,
    workspace_id: &str,
    member_user_id: &str,
) -> Result<WorkspaceKeyGrant> {
    let recipient_public = decode_grant_bytes(recipient_public_key)?;
    let mut ephemeral_bytes = [0_u8; 32];
    getrandom::fill(&mut ephemeral_bytes).map_err(|_| Error::RandomnessUnavailable)?;
    let ephemeral_secret = x25519_dalek::StaticSecret::from(ephemeral_bytes);
    ephemeral_bytes.zeroize();
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let shared = ephemeral_secret.diffie_hellman(&x25519_dalek::PublicKey::from(recipient_public));
    if !shared.was_contributory() {
        return Err(Error::InvalidMemberIdentityKey);
    }
    let key_id = key.key_id().to_string();
    let cipher = grant_cipher(shared.as_bytes(), workspace_id, member_user_id, &key_id)?;
    let mut nonce = [0_u8; 24];
    getrandom::fill(&mut nonce).map_err(|_| Error::RandomnessUnavailable)?;
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: key.expose_bytes().as_slice(),
                aad: &grant_aad(workspace_id, member_user_id, &key_id),
            },
        )
        .map_err(|_| Error::InvalidWorkspaceKeyGrant)?;
    Ok(WorkspaceKeyGrant {
        key_id,
        ephemeral_public_key: URL_SAFE_NO_PAD.encode(ephemeral_public.as_bytes()),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}

/// Every workspace key generation a member can still read.
///
/// Rotation mints a new key and leaves earlier ones in place, so history stays
/// readable while writes move to the newest generation.
pub struct WorkspaceKeyring {
    active: WorkspaceKey,
    retired: Vec<WorkspaceKey>,
}

impl WorkspaceKeyring {
    pub fn new(active: WorkspaceKey) -> Self {
        Self {
            active,
            retired: Vec::new(),
        }
    }

    /// Adds an older generation. Re-adding the active key or a duplicate is a
    /// no-op so callers can replay grant lists without bookkeeping.
    pub fn insert_retired(&mut self, key: WorkspaceKey) {
        if self.get(key.key_id()).is_some() {
            return;
        }
        self.retired.push(key);
    }

    pub fn active(&self) -> &WorkspaceKey {
        &self.active
    }

    pub fn get(&self, key_id: &str) -> Option<&WorkspaceKey> {
        if self.active.key_id() == key_id {
            return Some(&self.active);
        }
        self.retired.iter().find(|key| key.key_id() == key_id)
    }

    pub fn open_field(
        &self,
        workspace_id: &str,
        record_id: &str,
        payload: &str,
    ) -> Result<crate::OpenedField> {
        let envelope: Envelope =
            serde_json::from_str(payload).map_err(|_| Error::InvalidPayload)?;
        self.get(&envelope.key_id)
            .ok_or(Error::UnknownKey)?
            .open_field(workspace_id, record_id, payload)
    }
}

impl RecoveryKey {
    pub fn member_identity_key(&self) -> Result<MemberIdentityKey> {
        let hkdf = Hkdf::<Sha256>::new(Some(MEMBER_IDENTITY_KDF_SALT), self.expose_bytes());
        let mut bytes = [0_u8; 32];
        hkdf.expand(MEMBER_IDENTITY_INFO, &mut bytes)
            .map_err(|_| Error::InvalidMemberIdentityKey)?;
        Ok(MemberIdentityKey(bytes))
    }
}

fn decode_grant_bytes(value: &str) -> Result<[u8; 32]> {
    URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| Error::InvalidMemberIdentityKey)?
        .try_into()
        .map_err(|_| Error::InvalidMemberIdentityKey)
}

fn grant_cipher(
    shared_secret: &[u8; 32],
    workspace_id: &str,
    member_user_id: &str,
    key_id: &str,
) -> Result<XChaCha20Poly1305> {
    if workspace_id.trim().is_empty() || member_user_id.trim().is_empty() || key_id.is_empty() {
        return Err(Error::InvalidWorkspaceKeyGrant);
    }
    let hkdf = Hkdf::<Sha256>::new(Some(GRANT_KDF_SALT), shared_secret);
    let mut key = Zeroizing::new([0_u8; 32]);
    hkdf.expand(
        &grant_aad(workspace_id, member_user_id, key_id),
        key.as_mut(),
    )
    .map_err(|_| Error::InvalidWorkspaceKeyGrant)?;
    Ok(XChaCha20Poly1305::new_from_slice(key.as_ref())
        .expect("XChaCha20Poly1305 accepts 32-byte keys"))
}

fn grant_aad(workspace_id: &str, member_user_id: &str, key_id: &str) -> Vec<u8> {
    let mut aad = Vec::new();
    for value in [
        GRANT_AAD_DOMAIN,
        workspace_id.as_bytes(),
        member_user_id.as_bytes(),
        key_id.as_bytes(),
    ] {
        aad.extend_from_slice(&(value.len() as u64).to_be_bytes());
        aad.extend_from_slice(value);
    }
    aad
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const WORKSPACE: &str = "workspace-shared";
    const ALICE: &str = "user-alice";
    const BOB: &str = "user-bob";
    const WRITER: &str = "00000000000000000000000000000001";

    fn recovery_key(seed: u8) -> RecoveryKey {
        RecoveryKey::parse(&format!(
            "anarlog-e2ee-v1:{}",
            URL_SAFE_NO_PAD.encode([seed; 32])
        ))
        .unwrap()
    }

    fn seal(key: &WorkspaceKey, value: &str) -> crate::SealedField {
        key.seal_field(
            WORKSPACE,
            "sessions",
            "session-1",
            "title",
            WRITER,
            1,
            false,
            json!(value),
        )
        .unwrap()
    }

    #[test]
    fn shared_workspace_keys_are_random_per_generation() {
        let first = WorkspaceKey::generate().unwrap();
        let second = WorkspaceKey::generate().unwrap();

        assert_ne!(first.key_id(), second.key_id());
    }

    #[test]
    fn shared_keys_are_not_derivable_from_any_members_recovery_key() {
        let shared = WorkspaceKey::generate().unwrap();
        let derived = recovery_key(1).workspace_key(WORKSPACE).unwrap();

        assert_ne!(shared.key_id(), derived.key_id());
    }

    #[test]
    fn member_identities_are_deterministic_and_distinct() {
        let alice = recovery_key(1);
        let bob = recovery_key(2);

        assert_eq!(
            alice.member_identity_key().unwrap().public_key(),
            alice.member_identity_key().unwrap().public_key()
        );
        assert_ne!(
            alice.member_identity_key().unwrap().public_key(),
            bob.member_identity_key().unwrap().public_key()
        );
    }

    #[test]
    fn grants_hand_the_shared_key_to_another_member() {
        let key = WorkspaceKey::generate().unwrap();
        let bob = recovery_key(2).member_identity_key().unwrap();

        let grant = seal_workspace_key_for_member(&key, &bob.public_key(), WORKSPACE, BOB).unwrap();
        let opened = bob.open_workspace_key(WORKSPACE, BOB, &grant).unwrap();

        assert_eq!(opened.key_id(), key.key_id());

        // Bob can now read what an existing member wrote.
        let sealed = seal(&key, "Roadmap");
        let field = opened
            .open_field(WORKSPACE, &sealed.record_id, &sealed.payload)
            .unwrap();
        assert_eq!(field.value, json!("Roadmap"));
    }

    #[test]
    fn grants_are_bound_to_workspace_recipient_and_key_id() {
        let key = WorkspaceKey::generate().unwrap();
        let bob = recovery_key(2).member_identity_key().unwrap();
        let grant = seal_workspace_key_for_member(&key, &bob.public_key(), WORKSPACE, BOB).unwrap();

        assert!(
            bob.open_workspace_key("other-workspace", BOB, &grant)
                .is_err()
        );
        assert!(bob.open_workspace_key(WORKSPACE, ALICE, &grant).is_err());

        let mut relabelled = grant.clone();
        relabelled.key_id = WorkspaceKey::generate().unwrap().key_id().to_string();
        assert!(bob.open_workspace_key(WORKSPACE, BOB, &relabelled).is_err());
    }

    #[test]
    fn grants_are_useless_to_everyone_except_their_recipient() {
        let key = WorkspaceKey::generate().unwrap();
        let bob = recovery_key(2).member_identity_key().unwrap();
        let mallory = recovery_key(3).member_identity_key().unwrap();
        let grant = seal_workspace_key_for_member(&key, &bob.public_key(), WORKSPACE, BOB).unwrap();

        assert!(mallory.open_workspace_key(WORKSPACE, BOB, &grant).is_err());
    }

    #[test]
    fn tampered_grants_fail_closed() {
        let key = WorkspaceKey::generate().unwrap();
        let bob = recovery_key(2).member_identity_key().unwrap();
        let grant = seal_workspace_key_for_member(&key, &bob.public_key(), WORKSPACE, BOB).unwrap();

        let mut tampered = grant.clone();
        tampered.ciphertext = URL_SAFE_NO_PAD.encode([0_u8; 48]);
        assert!(bob.open_workspace_key(WORKSPACE, BOB, &tampered).is_err());

        let mut swapped = grant.clone();
        swapped.ephemeral_public_key = URL_SAFE_NO_PAD.encode([0_u8; 32]);
        assert!(matches!(
            bob.open_workspace_key(WORKSPACE, BOB, &swapped),
            Err(Error::InvalidWorkspaceKeyGrant)
        ));
    }

    #[test]
    fn rejects_noncontributory_recipient_public_key() {
        let key = WorkspaceKey::generate().unwrap();

        assert!(matches!(
            seal_workspace_key_for_member(
                &key,
                &URL_SAFE_NO_PAD.encode([0_u8; 32]),
                WORKSPACE,
                BOB
            ),
            Err(Error::InvalidMemberIdentityKey)
        ));
    }

    #[test]
    fn removing_a_member_cuts_them_off_from_writes_after_rotation() {
        let first = WorkspaceKey::generate().unwrap();
        let bob = recovery_key(2).member_identity_key().unwrap();
        let bobs_key = bob
            .open_workspace_key(
                WORKSPACE,
                BOB,
                &seal_workspace_key_for_member(&first, &bob.public_key(), WORKSPACE, BOB).unwrap(),
            )
            .unwrap();

        let before = seal(&first, "Before removal");

        // Bob is removed: the workspace rotates and he receives no new grant.
        let second = WorkspaceKey::generate().unwrap();
        let after = seal(&second, "After removal");

        assert!(
            bobs_key
                .open_field(WORKSPACE, &after.record_id, &after.payload)
                .is_err(),
            "a removed member must not read writes made after rotation"
        );
        assert_eq!(
            bobs_key
                .open_field(WORKSPACE, &before.record_id, &before.payload)
                .unwrap()
                .value,
            json!("Before removal"),
            "history stays readable for whoever already held the old key"
        );
    }

    #[test]
    fn keyrings_read_across_generations_and_reject_unknown_keys() {
        let first = WorkspaceKey::generate().unwrap();
        let second = WorkspaceKey::generate().unwrap();
        let stranger = WorkspaceKey::generate().unwrap();

        let old = seal(&first, "Old");
        let new = seal(&second, "New");
        let foreign = seal(&stranger, "Foreign");

        let mut keyring = WorkspaceKeyring::new(second.clone());
        keyring.insert_retired(first.clone());
        keyring.insert_retired(first);
        keyring.insert_retired(second);

        assert_eq!(keyring.active().key_id(), keyring.active().key_id());
        assert_eq!(
            keyring
                .open_field(WORKSPACE, &old.record_id, &old.payload)
                .unwrap()
                .value,
            json!("Old")
        );
        assert_eq!(
            keyring
                .open_field(WORKSPACE, &new.record_id, &new.payload)
                .unwrap()
                .value,
            json!("New")
        );
        assert!(matches!(
            keyring.open_field(WORKSPACE, &foreign.record_id, &foreign.payload),
            Err(Error::UnknownKey)
        ));
    }

    #[test]
    fn rotation_rewraps_for_remaining_members_only() {
        let members = [(ALICE, recovery_key(1)), (BOB, recovery_key(2))];
        let removed = (String::from("user-mallory"), recovery_key(3));

        let rotated = WorkspaceKey::generate().unwrap();
        for (user_id, recovery) in &members {
            let identity = recovery.member_identity_key().unwrap();
            let grant =
                seal_workspace_key_for_member(&rotated, &identity.public_key(), WORKSPACE, user_id)
                    .unwrap();
            assert_eq!(
                identity
                    .open_workspace_key(WORKSPACE, user_id, &grant)
                    .unwrap()
                    .key_id(),
                rotated.key_id()
            );
        }

        // The removed member has no grant for the rotated generation, and a
        // grant minted for someone else does not help them.
        let mallory = removed.1.member_identity_key().unwrap();
        let alice_identity = members[0].1.member_identity_key().unwrap();
        let alice_grant =
            seal_workspace_key_for_member(&rotated, &alice_identity.public_key(), WORKSPACE, ALICE)
                .unwrap();
        assert!(
            mallory
                .open_workspace_key(WORKSPACE, &removed.0, &alice_grant)
                .is_err()
        );
    }
}
