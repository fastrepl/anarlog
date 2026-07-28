---
name: qa-cli-mcp-api
description: QA-test Anarlog's programmatic interfaces end to end: the CLI, local REST API, stdio MCP server, hosted Cloud API, and remote MCP server. Use before a release or after changes to agent-access DTOs, API auth, API keys, snapshots, pagination, exports, webhooks, or MCP tools.
---

# QA: CLI, MCP, and API

Treat this as a release gate for Anarlog's programmatic interfaces. Automated
tests are necessary but do not replace the live smoke tests below.

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
- In a GitButler workspace, use the selected branch tip from
  `but status --format json`; do not use the synthetic workspace `HEAD` as
  release provenance.
- Mark a lane `BLOCKED`, not `PASS`, when its required deployment, account,
  fixture, or client is unavailable.

## Required fixture

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

## Automated baseline

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
to pass without updating snapshots or generated clients during the gate.

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

## Local REST API

Launch the exact desktop candidate, enable **Settings → Developers → Local
API**, create a short-lived local key, and use `http://127.0.0.1:33443`.

1. `GET /health` must work without authentication.
2. A protected route with no key, a malformed key, and a revoked key must
   return `401` with the documented JSON error envelope.
3. Exercise every read endpoint:
   - `GET /v1/meetings`
   - `GET /v1/meetings/{meeting_id}`
   - `GET /v1/meetings/{meeting_id}/transcript`
   - `GET /v1/meetings/{meeting_id}/history`
   - `GET /v1/meetings/{meeting_id}/export?format=json`
   - `GET /v1/meetings/{meeting_id}/export?format=markdown`
4. Repeat query, series, limit, offset, missing-ID, and transcript boundary
   cases from the CLI lane.
5. Confirm the server listens only on loopback and becomes unavailable when
   disabled or when the desktop app exits.
6. Restart the desktop app. The enabled state and non-revoked key must retain
   their documented behavior.
7. Create a temporary webhook receiver, subscribe to each documented event,
   and trigger a test, meeting completion, and note enhancement.
   - Verify the event name, delivery ID, timestamp, body, and HMAC-SHA256
     signature over the exact raw body.
   - Verify retry behavior with one deliberate transient failure.
   - Delete the webhook and prove that no later delivery arrives.

## Local stdio MCP

Start `anarlog mcp` through a real MCP client over stdio. Do not validate this
lane by invoking server handlers directly.

1. Initialize the protocol and run `tools/list`.
2. Require exactly these tools:
   - `list_meetings`
   - `get_meeting`
   - `get_meeting_transcript`
   - `get_recurring_meeting_history`
3. Call every tool against the fixture. Verify query and series filters,
   two-page transcript/history traversal, missing IDs, and invalid arguments.
4. Run `resources/list`, `resources/templates/list`, and `resources/read` for a
   meeting, transcript page, and recurring series.
5. Require read-only, non-destructive, and idempotent tool annotations.
6. Compare the returned values with the CLI and local REST lanes.
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
   filters, pagination, error cases, and exports as the local REST lane.
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

Connect a real Streamable HTTP MCP client to the deployed `/mcp` endpoint with
the short-lived cloud key.

1. Initialize a session, list tools, and require the same four-tool contract as
   local MCP.
2. Call every tool and traverse at least two transcript/history pages.
3. Compare its structured results with the hosted REST responses.
4. Repeat initialization or a tool call with no key, a malformed key, a
   revoked key, an expired account, and after opt-out. Require the same
   documented auth and entitlement semantics as REST.
5. Connect at least one supported agent client using the documented setup and
   ask it to identify the marked meeting, summarize it, and cite a transcript
   detail. Verify the answer against the fixture.
6. Close the client and confirm the server releases the session cleanly.

## Cross-surface parity

For the marked meeting, compare CLI, local REST, local MCP, hosted REST, and
remote MCP:

| Field | Required parity |
| --- | --- |
| Meeting | ID, title, kind, status, timestamps, timezone, language, series |
| Documents | canonical note, summary titles and markdown |
| People | participants and organizations |
| Actions | text, assignee, completion state |
| Transcript | text, word order, timestamps, speakers, page boundaries |
| History | IDs, newest-first ordering, pagination |
| Errors | stable code, appropriate protocol status, no secret leakage |

Local and hosted values must match after backfill settles. Hosted payloads must
not contain local paths, audio paths, file paths, control characters, secrets,
or fields outside the disclosed server-readable copy.

## Reporting

Produce one table with rows for:

- automated baseline;
- CLI;
- local REST;
- local webhooks;
- local stdio MCP;
- hosted REST;
- remote MCP;
- lifecycle and account isolation;
- privacy purge;
- cross-surface parity.

Use `PASS`, `FAIL`, or `BLOCKED` with a one-line evidence note. Include the
candidate and deployment identifiers, fixture marker, clients tested, and
redacted response artifact locations. List every skipped negative case.

Do not approve the release when any required row is `FAIL` or `BLOCKED`.
