# Enterprise access control and remote deletion

ANLG-133. Spec only.

## v1 admin surface

Roles: workspace owner, admin, member. Owners transfer/delete the workspace. Admins manage members, invitations, share policy, retention, SSO/SCIM domain, and usage overview. Members read allowed share scopes and honor them in the share panel.

Non-goals for v1: folder ACLs, per-note classification labels, MDM APIs, agent-specific roles, org-wide audit log UI beyond share access events.

## Revocation

- Remove member or SCIM deprovision: membership `deleted_at`, `sync_devices` deleted, E2EE workspace key rotation (ANLG-211).
- Sign out everywhere revokes refresh tokens.
- Offline devices keep local ciphertext until they next sync; they cannot decrypt rotated workspace keys.

## Remote deletion

Retention job `enforce_workspace_retention` soft-deletes expired `session_shares` and drops snapshots. CloudSync then stops serving those rows; local mirrors delete on next sync. Hard technical constraint: a device that never comes online cannot be forced to wipe. v1 states that limitation instead of pretending MDM exists.

Audit: `session_access_events` is the share access log. Org-wide admin audit of exports/deletes is later.

## Policy controls in v1

Allowed share scopes, default scope, retention days, model-training opt-out, consent notification default, require SSO.
