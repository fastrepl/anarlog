# Observability

This document is the living spec for observability in this repo.

It defines:

- how we use OpenTelemetry
- how data is expected to appear in Honeycomb
- what `x-request-id` means
- how we use Sentry tags, contexts, and user identity
- which attribute names are allowed

If a change introduces new tracing fields, propagation behavior, or Sentry tagging conventions, update this file in the same change.

## Scope

This repo has multiple binaries and runtime surfaces, but the same conventions apply everywhere:

- `apps/api` is one OTEL service
- `apps/desktop` is one Sentry/desktop service
- internal route groups or modules are not separate OTEL services
- internal logical breakdowns use `hyprnote.subsystem`

Current canonical subsystem values include:

- `edge`
- `llm`
- `stt`
- `subscription`

## Observability Stack

We use three separate concepts:

1. OpenTelemetry
   - canonical tracing model
   - canonical attribute naming model
   - canonical propagation model
2. Honeycomb
   - primary trace analysis backend
   - expects OTEL resources, spans, and high-cardinality fields
3. Sentry
   - error reporting and local debugging context
   - should mirror OTEL naming where practical

`x-request-id` is not trace propagation. It is a separate request-correlation mechanism.

## Resource And Service Model

### Canonical OTEL resource attributes

Every process should set:

- `service.namespace = "hyprnote"`
- `service.name = <logical process name>`
- `service.version`
- `deployment.environment`

Current canonical service names:

- API: `api`
- Desktop: `desktop`

### What counts as a service

Use one `service.name` per deployable/runtime process.

Do not create separate `service.name` values for:

- axum route groups
- internal modules
- handler categories
- provider adapters

For example, `edge`, `llm`, `stt`, and `subscription` inside `apps/api` are not separate services. They are subsystems within the `api` service.

### Subsystems

Use:

- `hyprnote.subsystem`

Examples:

- API ingress span: `hyprnote.subsystem = "edge"`
- LLM handler span: `hyprnote.subsystem = "llm"`
- STT websocket/session spans: `hyprnote.subsystem = "stt"`

Do not use a bare `service` span field for this.

## Propagation

### Canonical propagation format

For distributed tracing, use W3C Trace Context:

- `traceparent`
- `baggage` only when intentionally needed

Sentry headers may also exist:

- `sentry-trace`
- `baggage`

But OTEL trace stitching must work through W3C propagation.

### Rules

Inbound requests:

- extract remote W3C trace context
- set the server span parent from the extracted context

Outbound requests:

- inject current W3C trace context

Do not use custom trace propagation headers when W3C exists.

### Current implementation

Rust shared helpers live in:

- [`crates/observability/src/lib.rs`](/Users/yujonglee/dev/char/crates/observability/src/lib.rs)

API ingress extraction and root HTTP span setup live in:

- [`apps/api/src/main.rs`](/Users/yujonglee/dev/char/apps/api/src/main.rs)

Desktop request header creation lives in:

- [`apps/desktop/src/shared/utils.ts`](/Users/yujonglee/dev/char/apps/desktop/src/shared/utils.ts)
- [`apps/desktop/src/ai/traced-fetch.ts`](/Users/yujonglee/dev/char/apps/desktop/src/ai/traced-fetch.ts)
- [`apps/desktop/src/auth/context.tsx`](/Users/yujonglee/dev/char/apps/desktop/src/auth/context.tsx)

### Baggage policy

Do not put user identity or device identifiers into baggage by default.

In particular, do not propagate:

- `enduser.id`
- `enduser.pseudo.id`
- device fingerprints

as baggage unless there is an explicit need and a privacy review.

## Request ID

### Meaning

`x-request-id` is a correlation ID for support, logs, and local debugging.

It is not:

- a trace ID
- a span ID
- a substitute for `traceparent`

### Rules

- generate it once at ingress if missing
- forward it unchanged when useful
- record it as `hyprnote.request.id`
- keep it semantically separate from OTEL trace context

Never do this:

- `x-request-id = trace_id`
- reconstruct trace relationships from `x-request-id`

### Current implementation

API ingress uses request-id middleware and records the value on the root span:

- [`apps/api/src/main.rs`](/Users/yujonglee/dev/char/apps/api/src/main.rs)

Desktop client requests add `x-request-id` separately from `traceparent`:

- [`apps/desktop/src/ai/traced-fetch.ts`](/Users/yujonglee/dev/char/apps/desktop/src/ai/traced-fetch.ts)
- [`apps/desktop/src/auth/context.tsx`](/Users/yujonglee/dev/char/apps/desktop/src/auth/context.tsx)

## Naming Rules

### Rule 1: Prefer OTEL semantic conventions

If OTEL defines a field for the concept, use the OTEL field.

Examples:

- `service.namespace`
- `service.name`
- `http.request.method`
- `http.route`
- `http.response.status_code`
- `url.path`
- `enduser.id`
- `enduser.pseudo.id`
- `error.type`
- `error.message`
- `error.code`
- `service.peer.name`
- `gen_ai.operation.name`
- `gen_ai.provider.name`
- `gen_ai.request.model`
- `gen_ai.response.model`
- `gen_ai.response.id`
- `gen_ai.usage.input_tokens`
- `gen_ai.usage.output_tokens`

### Rule 2: Custom fields must use `hyprnote.*`

If OTEL does not define a field, use:

- `hyprnote.*`

Do not use:

- `app.*`
- bare ad hoc names like `service`, `provider`, `status`, `session_id`, `user_id`

We avoid `app.*` because OpenTelemetry owns that namespace.

### Rule 3: One concept, one name

If a concept already has an approved key, reuse it everywhere:

- OTEL spans
- tracing logs/events
- Sentry tags
- Sentry contexts

Do not rename the same concept differently per backend.

## Canonical Field Families

### Identity

- `enduser.id`
- `enduser.pseudo.id`

Use:

- `enduser.id` for authenticated user ID
- `enduser.pseudo.id` for device fingerprint or other stable pseudonymous device identity

### Request and duration

- `hyprnote.request.id`
- `hyprnote.duration_ms`
- `hyprnote.retry.delay_ms`
- `hyprnote.timeout_s`
- `hyprnote.timeout.elapsed`

### HTTP and routing

- `http.request.method`
- `http.route`
- `http.response.status_code`
- `url.path`
- `url.full` when needed
- `otel.kind`
- `otel.name`

Ingress HTTP spans should be `otel.kind = "server"`.

### LLM

Use OTEL GenAI fields where available:

- `gen_ai.operation.name`
- `gen_ai.provider.name`
- `gen_ai.request.model`
- `gen_ai.response.model`
- `gen_ai.response.id`
- `gen_ai.usage.input_tokens`
- `gen_ai.usage.output_tokens`

Use `hyprnote.*` for Hyprnote-specific request metadata:

- `hyprnote.gen_ai.request.streaming`
- `hyprnote.gen_ai.request.message_count`
- `hyprnote.gen_ai.request.model_candidate_count`
- `hyprnote.gen_ai.request.tool_calling`
- `hyprnote.task.name`

### STT and audio

Use:

- `hyprnote.stt.provider.name`
- `hyprnote.stt.routing_strategy`
- `hyprnote.stt.model`
- `hyprnote.stt.language_codes`
- `hyprnote.stt.language_code`
- `hyprnote.stt.session.id`
- `hyprnote.stt.job.id`
- `hyprnote.stt.provider_session.id`
- `hyprnote.stt.provider_session.duration_s`
- `hyprnote.stt.provider_session.expires_at`
- `hyprnote.stt.provider.error_code`
- `hyprnote.audio.sample_rate_hz`
- `hyprnote.audio.channel_count`
- `hyprnote.audio.channel_index`
- `hyprnote.audio.size_bytes`
- `hyprnote.audio.duration_s`
- `hyprnote.audio.device`

### Vendor-specific fields

Keep vendor-specific fields namespaced:

- `hyprnote.supabase.*`
- `hyprnote.stripe.*`
- `hyprnote.connection.*`
- `hyprnote.integration.*`
- `hyprnote.bot.*`

Always prefer `service.peer.name` for the downstream system name.

### Payload and debug-only fields

If raw payload capture is necessary for debug logs, use:

- `hyprnote.payload.raw`
- `hyprnote.http.response.body`
- `hyprnote.http.body_preview`

Do not put large raw payloads on high-volume spans by default.

## Honeycomb Conventions

### Service breakdown

Honeycomb service views come from OTEL resource attributes, especially:

- `service.name`

Because of that:

- `apps/api` must stay one Honeycomb service: `api`
- internal analysis should use `hyprnote.subsystem`

### High cardinality

Honeycomb handles high-cardinality fields well. IDs are allowed when they help debugging.

Good high-cardinality examples:

- `hyprnote.request.id`
- `enduser.id`
- `enduser.pseudo.id`
- `gen_ai.response.id`
- `hyprnote.stt.job.id`
- provider session IDs

Do not avoid useful IDs just because they are high cardinality.

### Root span quality

Server entry spans should:

- have a remote parent if the request carries one
- set `otel.kind = "server"`
- set `otel.name`
- record HTTP route and status

### Span field declaration rule

When using `tracing`, declare fields up front if you plan to `record` them later.

This matters for:

- `#[tracing::instrument(fields(...))]`
- `tracing::info_span!(...)`

If a field is not declared on span creation, later `span.record(...)` calls will not create a new OTEL attribute.

## Sentry Conventions

### Purpose

Sentry is for:

- errors
- crash reports
- request-local debugging context

It is not the canonical trace schema. OTEL is.

### Tag naming

Reuse OTEL names when possible.

Canonical Sentry tags include:

- `service.namespace`
- `service.name`
- `enduser.id`
- `enduser.pseudo.id`
- `http.response.status_code`
- `error.type`
- `gen_ai.provider.name`
- `gen_ai.request.model`
- `hyprnote.gen_ai.request.streaming`
- `hyprnote.stt.provider.name`
- `hyprnote.stt.routing_strategy`
- `hyprnote.stt.model`
- `hyprnote.stt.language_codes`

### Context naming

Use contexts for structured objects that are too rich for tags.

Canonical context names include:

- `gen_ai.request`
- `gen_ai.response`
- `hyprnote.stt.request`
- `hyprnote.enduser.claims`
- `hyprnote.session`

### Sentry user

Set `scope.set_user(...)` when identity is available.

API:

- authenticated requests use the auth subject as the Sentry user ID

Desktop:

- use a pseudonymous device identity when no authenticated user exists yet

### Alignment rule

Do not invent Sentry-only field names for concepts that already exist in OTEL unless Sentry forces it.

Good:

- `enduser.id`
- `service.name`
- `error.type`

Bad:

- `user_id`
- `service`
- `upstream.status`
- `llm.model` when `gen_ai.request.model` already exists

## Error Conventions

Use:

- `error.type` for machine-readable classification
- `error.message` for the display/debug message
- `error.code` when an external or protocol code exists

Examples:

- provider returned a structured error code
- timeout class
- invalid payload class

Avoid ad hoc variants such as:

- `message`
- `error`
- `error_type`
- `error_code`

## Header Conventions

Canonical headers used in this repo:

- `traceparent`
- `baggage`
- `sentry-trace`
- `x-request-id`
- `x-device-fingerprint`

Meaning:

- `traceparent`: canonical trace propagation
- `baggage`: optional propagation metadata, usually originating from Sentry on desktop HTTP requests
- `sentry-trace`: Sentry tracing integration
- `x-request-id`: request correlation only
- `x-device-fingerprint`: local pseudonymous device identifier

## What To Do When Adding Instrumentation

1. Decide whether the concept already has an OTEL semantic convention.
2. If yes, use the OTEL field name.
3. If no, add a `hyprnote.*` field.
4. If the field will be recorded later on a span, declare it at span creation.
5. If the code crosses a network boundary, extract or inject W3C trace context.
6. If request correlation is needed, keep `x-request-id` separate from trace propagation.
7. Mirror the most important fields into Sentry tags or contexts using the same names.
8. Update this file if you introduce a new field family or a new rule.

## Anti-Patterns

Do not do any of the following:

- per-span `service = "llm"` style fields
- `x-request-id = trace_id`
- custom propagation instead of W3C trace context
- `app.*` custom fields
- different names for the same concept across OTEL and Sentry
- stuffing user identity into baggage by default
- creating new span attributes with `span.record` without declaring them first
- using route groups as separate Honeycomb services

## Current Reference Points

The current implementation that this spec describes is centered in:

- [`apps/api/src/observability.rs`](/Users/yujonglee/dev/char/apps/api/src/observability.rs)
- [`apps/api/src/main.rs`](/Users/yujonglee/dev/char/apps/api/src/main.rs)
- [`apps/api/src/auth.rs`](/Users/yujonglee/dev/char/apps/api/src/auth.rs)
- [`crates/observability/src/lib.rs`](/Users/yujonglee/dev/char/crates/observability/src/lib.rs)
- [`crates/llm-proxy/src/handler/mod.rs`](/Users/yujonglee/dev/char/crates/llm-proxy/src/handler/mod.rs)
- [`crates/llm-proxy/src/handler/non_streaming.rs`](/Users/yujonglee/dev/char/crates/llm-proxy/src/handler/non_streaming.rs)
- [`crates/llm-proxy/src/handler/streaming.rs`](/Users/yujonglee/dev/char/crates/llm-proxy/src/handler/streaming.rs)
- [`crates/transcribe-proxy/src/routes/streaming/mod.rs`](/Users/yujonglee/dev/char/crates/transcribe-proxy/src/routes/streaming/mod.rs)
- [`crates/transcribe-proxy/src/routes/streaming/session.rs`](/Users/yujonglee/dev/char/crates/transcribe-proxy/src/routes/streaming/session.rs)
- [`apps/desktop/src/shared/utils.ts`](/Users/yujonglee/dev/char/apps/desktop/src/shared/utils.ts)
- [`apps/desktop/src/ai/traced-fetch.ts`](/Users/yujonglee/dev/char/apps/desktop/src/ai/traced-fetch.ts)
- [`apps/desktop/src/auth/context.tsx`](/Users/yujonglee/dev/char/apps/desktop/src/auth/context.tsx)
- [`apps/desktop/src-tauri/src/lib.rs`](/Users/yujonglee/dev/char/apps/desktop/src-tauri/src/lib.rs)

## Change Policy

Treat this document as normative.

If code and this file disagree:

- update the code to match this spec, or
- update this spec in the same change with a deliberate rationale

Do not let drift accumulate.
