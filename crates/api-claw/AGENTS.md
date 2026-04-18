# api-claw

User-scoped VM lifecycle on top of `crates/exedev`. Treat every user as
getting exactly one VM, running `apps/claw`.

## Layering

1. `crates/exedev` — typed SDK for `POST https://exe.dev/exec` and the VM
   auth proxy. No knowledge of users, no policy.
2. `crates/api-claw` (this crate) — deterministic naming, per-user keys,
   provision/suspend/resume/deprovision mechanics, VM-scoped token minting.
   **Does not know about billing.**
3. `crates/api-subscription` (or equivalent) — calls into `ClawManager`
   from webhook handlers. This is where policy lives (e.g. "on
   `invoice.payment_failed` → suspend").

Keep these layers one-way: never let billing leak into `exedev`, and never
let low-level exe.dev details leak out of `exedev::commands`.

## Conventions (encoded in code)

- **VM name**: `vm_name(UserId) = "claw-" + first 12 hex of sha256(user_id)`.
  DNS-safe for `<name>.exe.xyz`. Do not construct VM names any other way.
  There is no tag-based lookup because `ls` does not return tags.
- **One SSH key per user**, generated on first `provision` and added via
  `ssh-key add`. This gives us per-user revocation: removing the key
  invalidates every token derived from it without touching other users.
- **Token ctx** always includes `{"user_id": <id>}`. Extra fields go under
  the same object via `ClawCallOptions::with_ctx`. Claw reads this verbatim
  from the `X-ExeDev-Token-Ctx` header.
- **Token lifetime** is whatever the caller asks for (`ClawCallOptions::exp`).
  Prefer minutes, not days: tokens are re-minted on every `call`.
- **Suspension** = remove the user's SSH key (`ssh_key_remove`). **Resume**
  = re-add the same stored key. The VM keeps running in both states; only
  HTTPS access is gated.

## Adding new VM commands

If you need to drive a command that `exedev` doesn't wrap yet:

1. Add the typed method to `crates/exedev/src/commands.rs`. Cover response
   parsing with a fixture in `commands::tests` (copy a real response from
   a gated smoke test).
2. If the command participates in user lifecycle, expose it via
   `ClawManager`. Otherwise keep it on the raw client.

Never add a new `exec_raw` caller outside `exedev`; all command-string
construction and quoting must stay in one place.

## Testing

- `cargo test -p exedev --lib` — quoting, naming, token namespace, response
  deserialization. All offline.
- `EXEDEV_TOKEN=<exe0|exe1> cargo test -p exedev --test live_smoke` —
  hits the real API to confirm the typed models still match. Runs no-ops
  only (`whoami`, `ls`, `ssh-key list`).
- `cargo test -p api-claw --lib` — deterministic naming, keyring round-trips,
  token ctx shape.

Do not add tests that `provision` real VMs in CI; cost and state leak are
too high. Add a gated live test only if the flow being exercised genuinely
can't be unit-tested.

## What this crate does NOT do

- Bill or observe billing state.
- Store SSH keys durably. `InMemoryKeyring` is for tests; wire in a
  Supabase-backed `UserKeyring` impl in the caller.
- Talk to claw's application layer. See `apps/claw/src/http.rs` for the
  receiving side of `ClawManager::call`.
