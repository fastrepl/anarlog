---
name: release-telemetry
description: Review Anarlog product analytics and error reporting for releases, preserving consent and verifying shipped attribution, ingestion, and symbols for affected paths.
metadata:
  internal: true
---

# Release Telemetry

Review every release; implement and verify affected paths and known gaps. Inherit
scope and session authorization from [Release a New Version](../release-new-version/SKILL.md).
Do not turn a release review into an unrelated telemetry migration.

## Implementation map

- Desktop product events: `apps/desktop/src/analytics.ts`,
  `plugins/analytics/`, and `crates/analytics/`.
- Desktop errors and consent: `apps/desktop/src-tauri/src/lib.rs`,
  `plugins/tracing/`, and current privacy/settings wiring.
- Website: `apps/web/src/providers/posthog.tsx`, analytics/privacy helpers under
  `apps/web/src/lib/`, and `apps/web/src/telemetry.ts` for browser tracing.
- Hosted services: `apps/api/src/main.rs`, affected API/proxy instrumentation,
  and `apps/stripe/` when billing changes.
- Requested mobile releases: `apps/mobile/src/lib/analytics.ts` and the current
  mobile error-reporting/bootstrap path. Do not infer desktop coverage applies.
- Native symbols and web source maps: `desktop_cd.yaml`, `web_cd.yaml`, and their
  actions. Inspect the actual platform steps and destinations before checking uploads.

## Review and verification

1. Trace changed journeys through event/error production, consent/channel gates,
   sanitization, transport, and destination. Cover success, failure, cancellation,
   retries, and recovery where affected. Reuse existing instrumentation with
   evidence; implement required missing coverage before candidate freeze or record
   an explicit deferral and impact.
2. Keep product analytics consent separate from crash-reporting consent. Preserve
   opt-out, private-route/global-privacy gates, redaction, deduplication, and bounded
   asynchronous delivery. Never include note/transcript content, credentials, keys,
   or raw private identifiers as diagnostic fixtures. Telemetry failure must not
   block recording, saving, or recovery.
3. Run affected component tests using current CI commands, including opt-out,
   sanitization, retry and collector-failure cases. Verify `APP_VERSION`, channel,
   environment and serving revision reach events from the actual packaged or
   deployed runtime. Developer SDK initialization is not shipped evidence.
4. Verify affected PostHog ingestion and Sentry ingestion/symbolication against the
   shipped release and each deployed service revision. Use existing authorized
   test accounts and harmless synthetic diagnostics where supported; do not crash
   the user's working app or change their consent to obtain evidence. For disabled
   channels, verify no emission. Inspect matching native debug IDs/source maps;
   upload success alone does not prove a captured stack is symbolicated.
5. Keep mocks, successful transport, destination ingestion, and symbolication as
   distinct evidence. Empty queries do not prove no events occurred. If credentials
   or suitable runtime evidence are unavailable, report that specific unchecked
   result; do not claim success or silently add a new release dispatch.

Return coverage/update or reuse decisions, source/runtime versions, consent and
privacy checks, destination evidence, and pending gaps to the coordinator.
