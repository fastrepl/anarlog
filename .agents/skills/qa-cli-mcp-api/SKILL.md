---
name: qa-cli-mcp-api
description: Select and run explicitly requested, risk-based QA for Anarlog's CLI, webhooks, stdio MCP, hosted Cloud API, and remote MCP. Test only affected lanes unless comprehensive coverage is requested.
---

# QA: CLI, MCP, and API

Default to the smallest set of programmatic-interface lanes that can prove the
change. Automated tests are necessary but do not replace a live smoke test when
the changed boundary is only exercised by a real client or deployment.

This QA workflow is independent from releasing. A release request alone does
not invoke it, and its results do not approve or block a release.

## Choose the scope first

Inspect the exact branch, commit, or diff and map changed code to its direct
consumers before creating fixtures or credentials. In a GitButler workspace,
use `but status` and `but show <commit-or-branch>`; do not use the synthetic
workspace `HEAD` as the candidate or combine unrelated applied branches.

Find the original reproduction in the current or past Codex task, linked
issue, PR, support report, or regression test. State the selected lanes and the
reason for each before testing.

### Targeted regression mode (default)

Select lanes by behavior, not by the existence of this checklist:

- CLI parsing, output, or local DB access → CLI plus the closest contract tests.
- Webhook endpoints, signing, or delivery retries → the webhook cases only.
- Shared agent-access DTOs, exports, filtering, or pagination → every direct
  consumer, including hosted REST or remote MCP when affected, plus
  cross-surface parity only for the changed fields.
- Hosted auth, snapshots, entitlements, isolation, purge, or Supabase policy →
  the affected hosted REST lifecycle and negative cases.
- Local or remote MCP protocol/tool changes → that MCP lane and its direct
  transport/contract dependency.
- Shared hosted REST/MCP behavior → both hosted consumers, but not unrelated
  local surfaces.

Run the closest affected automated tests, the original live reproduction, and
only credible boundary cases. A Rust or shared-crate change does not trigger
all lanes unless every lane consumes the changed behavior. Create only the
minimum non-sensitive fixture required for the selected checks. If a required
deployment, account, fixture, or client is unavailable, mark that check
`BLOCKED`; do not substitute unrelated lanes. Stop when the mapped risks are
covered.

### Comprehensive Interface QA

Run every fixture, baseline, live lane, lifecycle case, privacy check, and
cross-surface comparison below only when the user explicitly requests full or
comprehensive programmatic-interface QA.

## Safety and evidence

- Use a dedicated QA account and non-sensitive fixture meetings.
- Use a Pro or trialing account for the hosted lane and a separate free or
  expired test account for entitlement checks.
- Never put API keys, JWTs, webhook secrets, database credentials, or personal
  meeting content in commands, screenshots, reports, or repository files.
  Load secrets into environment variables without echoing them.
- Store transient response bodies in a directory created with `mktemp -d`.
  Remove that directory and revoke all generated keys after the run.
- Record the exact candidate commit, app version, API deployment, Supabase
  migration version, operating system, and client versions.
- In a GitButler workspace, identify the selected branch tip with `but status`
  and inspect it with `but show <branch>`; do not use the synthetic workspace
  `HEAD` as the candidate identity.
- Mark a lane `BLOCKED`, not `PASS`, when its required deployment, account,
  fixture, or client is unavailable.

## Comprehensive QA Fixture

Create two completed QA meetings through the desktop app:

1. A standalone meeting whose title contains a unique run marker such as
   `agent-access-2026-07-28T120000Z`.
2. Two meetings in the same recurring series.

The fixture must include:

- a note and at least one generated summary;
- two participants and one action item;
- enough transcript words to require two pages when requested with a small
  limit;
- punctuation and non-ASCII text;
- a later edit to the title, note, or summary;
- one meeting that will be deleted during lifecycle testing.

Record the meeting IDs and expected visible values. Do not seed SQLite or
Postgres directly for the live happy-path tests.

## Comprehensive Automated Baseline

Run from the repository root:

```bash
cargo test -p anarlog-cli
cargo test -p tauri-plugin-local-api
cargo test -p api-cloud
cargo test -p api openapi::tests
cargo check -p api-client
supabase test db
pnpm -F desktop exec vitest run \
  src/cloud-api/client.test.ts \
  src/settings/developers/index.test.tsx
pnpm -F desktop typecheck
pnpm exec dprint check
```

Require the CLI and MCP contract snapshots, OpenAPI composition tests,
authentication tests, desktop account-switch tests, and database policy tests
to pass without updating snapshots or generated clients during the run.

If the change touches a shared DTO or generated client, regenerate it using the
repository command that owns the artifact, then require a clean second
generation. A generated diff after the second run is a failure.

## Local CLI

Build the candidate CLI with `cargo build -p anarlog-cli`, then use the built
binary for every step.

1. Run `anarlog --json doctor`.
   - PASS when it exits `0`, reports `schema_version: "1"`, uses command
     `doctor`, reports `ready: true`, and resolves the intended QA database.
2. Run `meetings list` with JSON output.
   - Verify default ordering, the unique title query, exact series filtering,
     limit/offset pagination, and an empty result.
3. For the recorded meeting ID, run:
   - `meetings get ID`
   - `meetings note ID --kind note`
   - `meetings note ID --kind summary`
   - `meetings note ID --kind all`
   - `meetings transcript ID` with at least two pages
   - `meetings history ID` with at least two pages
   - `meetings export ID --format markdown`
   - `meetings export ID --format json`
4. Require every JSON response to have the correct `schema_version`, `command`,
   `data`, and pagination fields. Following `next_offset` must produce no
   duplicates or gaps.
5. Export to a temporary file. A second export without `--force` must fail
   without changing the file; `--force` must replace it.
6. A missing meeting ID and an unreadable or incompatible database must return
   machine-readable errors, a nonzero exit, and no panic or partial export.

## Webhooks

Launch the exact desktop candidate and add a temporary receiver under
**Settings → Developers → Webhooks**.

1. Adding an endpoint must return a `whsec_` secret exactly once, and a
   non-HTTP URL must be rejected.
2. Trigger a test delivery, a meeting completion, and a note enhancement.
   - Verify the event name, delivery ID, timestamp, body, and HMAC-SHA256
     signature over the exact raw body.
   - Verify retry behavior with one deliberate transient failure.
3. Confirm the last-delivery status shown in settings matches the receiver.
4. Delete the webhook and prove that no later delivery arrives.
5. Quit the desktop app, complete no further work, and confirm no deliveries
   are queued or replayed on the next launch.

## Local stdio MCP

Start `anarlog mcp` through a real MCP client over stdio. Do not validate this
lane by invoking server handlers directly.

1. Connect with the current MCP lifecycle and run `tools/list`. Repeat the handshake with a legacy `2025-11-25` client.
2. Require exactly these tools:
   - `list_meetings`
   - `get_meeting`
   - `get_meeting_transcript`
   - `get_recurring_meeting_history`
   - `export_meeting`
   - `propose_summary_edit`
   - `propose_memo_edit`
   - `list_proposals`
   - `get_proposal`
   - `decline_proposal`
3. Call every tool against the fixture. Verify query and series filters,
   two-page transcript/history traversal, missing IDs, and invalid arguments.
4. Run `resources/list`, `resources/templates/list`, and `resources/read` for a
   meeting, transcript page, and recurring series.
5. Require read-only, non-destructive, and idempotent tool annotations.
6. Compare the returned values with the CLI lane.
7. Capture stdout and stderr separately. Stdout must contain only MCP protocol
   frames; diagnostics belong on stderr. Client shutdown must terminate the
   server without leaving a process behind.

## Hosted Cloud API

Use the deployed candidate API and Supabase migration with the Pro QA account.
Begin with **Cloud API & Connectors** disabled.

1. Before opt-in, confirm there are no server-readable snapshot rows for the
   account and a previously valid key returns `403 cloud_api_not_enabled`.
2. Enable the feature in the desktop UI and wait for backfill to finish.
3. Create a short-lived cloud key and exercise the same read endpoints,
   filters, pagination, error cases, and exports as the CLI lane.
4. Require:
   - no key, malformed key, and revoked key → `401`;
   - free or expired account → `403 subscription_required`;
   - disabled account → `403 cloud_api_not_enabled`;
   - invalid input → `400 invalid_request`;
   - missing meeting → `404 not_found`;
   - request burst above the documented quota → `429` with `retry-after`.
5. Edit the fixture locally and confirm the hosted result changes. Delete the
   lifecycle fixture and confirm the hosted endpoint returns `404`.
6. Sign the desktop into another account before a queued upload or retry can
   complete. No snapshot from the first account may appear in the second.
7. Disable the feature.
   - PASS when all server-readable snapshots are purged, the cloud key returns
     `cloud_api_not_enabled`, local data remains, and normal encrypted sync
     data is unchanged.
8. Re-enable and confirm a fresh backfill restores only currently existing
   meetings. Revoke the QA key when finished.

## Remote MCP

Connect a real Streamable HTTP MCP client to the deployed `/mcp` endpoint.

1. Use `server/discover` with MCP `2026-07-28`, list tools without creating a
   session, and require exactly `list_meetings`, `get_meeting`,
   `get_meeting_transcript`, `get_recurring_meeting_history`, and
   `export_meeting`. Verify the required protocol metadata and standard HTTP
   headers on every modern request. Repeat discovery with a legacy
   `2025-11-25` stateless client to prove compatibility.
2. Call every tool and traverse at least two transcript/history pages.
3. Compare its structured results with the hosted REST responses.
4. Repeat initialization or a tool call with no credential, a malformed key, a
   revoked key, an expired account, and after opt-out. Unauthenticated MCP
   requests must return `WWW-Authenticate` pointing at
   `https://api.anarlog.so/.well-known/oauth-protected-resource/mcp`.
5. When OAuth is affected, complete MCP OAuth 2.1 discovery and consent from at
   least one documented host (Claude Code, Cursor, ChatGPT/Codex, or Copilot).
   Confirm the consent screen is Anarlog's `/oauth/consent` route, the issued
   token is bound to `https://api.anarlog.so/mcp`, and a tool call then reads
   the marked meeting. Repeat with a static `anl_` key for hosts that cannot
   complete OAuth.
6. Connect at least one supported agent client using the documented setup and
   ask it to identify the marked meeting, summarize it, and cite a transcript
   detail. Verify the answer against the fixture.
7. Close the client and confirm the server releases the session cleanly.

## Cross-surface parity

For the marked meeting, compare CLI, local MCP, hosted REST, and remote MCP:

| Field      | Required parity                                                 |
| ---------- | --------------------------------------------------------------- |
| Meeting    | ID, title, kind, status, timestamps, timezone, language, series |
| Documents  | canonical note, summary titles and markdown                     |
| People     | participants and organizations                                  |
| Actions    | text, assignee, completion state                                |
| Transcript | text, word order, timestamps, speakers, page boundaries         |
| History    | IDs, newest-first ordering, pagination                          |
| Errors     | stable code, appropriate protocol status, no secret leakage     |

Local and hosted values must match after backfill settles. Hosted payloads must
not contain local paths, audio paths, file paths, control characters, secrets,
or fields outside the disclosed server-readable copy.

## Reporting

For targeted mode, report the candidate branch/commit, base, selected lane and
risk, `PASS`/`FAIL`/`BLOCKED`, and a one-line evidence note. List unrelated
lanes once as `NOT APPLICABLE`, and say explicitly that the result is not
comprehensive interface QA.

For comprehensive mode, produce one table with rows for:

- automated baseline;
- CLI;
- webhooks;
- local stdio MCP;
- hosted REST;
- remote MCP;
- lifecycle and account isolation;
- privacy purge;
- cross-surface parity.

Use `PASS`, `FAIL`, or `BLOCKED` with a one-line evidence note. Include the
candidate and deployment identifiers, fixture marker, clients tested, and
redacted response artifact locations. List every skipped negative case.

Any required `FAIL` or `BLOCKED` result means comprehensive QA did not pass.
Report that outcome without inferring release approval or blocking.
