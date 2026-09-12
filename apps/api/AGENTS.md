```bash
infisical export \
  --env=dev \
  --secret-overriding=false \
  --format=dotenv \
  --output-file="apps/api/.env" \
  --projectId=87dad7b5-72a6-4791-9228-b3b86b169db1 \
  --path="/anarlog/ai"
```

`/anarlog/ai` is the API runtime view. Its Nango entries should reference the
source secrets in `/anarlog/nango` rather than requiring API jobs to export a
second secret path.

## Service runtimes

`ANARLOG_SERVICE` selects the routes started by the shared API binary:

| Value | Routes |
| --- | --- |
| `all` (default) | Existing combined API, preserving current client URLs |
| `ai` | Transcription, LLM, research, diarization, and STT callbacks |
| `sync` | Sync, devices, attachments, sharing, hosted meeting API/MCP |
| `core` | Nango and integration APIs, account deletion, SCIM |
| `billing` | Trial/subscription API under the existing `/subscription`, `/rpc`, and `/billing` aliases |

Stripe webhook processing remains in `apps/stripe`; the Rust billing runtime
is the extracted subscription API, not a replacement for that webhook handler.
Core retains the existing subscription configuration for account deletion and SCIM.

All roles require Supabase configuration. Only `ai` and `all` require
`OPENROUTER_API_KEY` and `API_BASE_URL`; the sync runtime requires its shared-note
email configuration. Optional integration groups are validated only in their
owning roles. `ANARLOG_ATTACHMENT_BACKUP_GC_ENABLED` is accepted only by `core`
and `all`. Assign cleanup to one deployment during migration.

Role selection does not change Fly routing or provision applications. Keep
existing URLs working while clients and webhook providers migrate.

Standalone profiles are `fly.ai.toml` (`anarlog-inference`),
`fly.sync.toml` (`anarlog-sync`), `fly.core.toml` (`anarlog-core`), and
`fly.billing.toml` (`anarlog-billing-api`). The default `fly.toml` and
`fly.gateway.toml` route the existing `anarlog-ai` URLs to these services. `fly.legacy.toml` preserves legacy `hyprnote-ai` URLs.
Core owns durable cleanup; other profiles disable it. Never transfer cleanup
ownership until the previous owner's worker has stopped.

`/health/ready/{service}` verifies the expected runtime role, configuration of
its primary subsystems, and that it is not draining. The combined role uses
`api`; Rust billing uses `billing-api`. These checks do not probe external
providers or prove request continuity. Keep dependency smoke tests and active
traffic deploy/rollback tests as separate rollout gates.

The drain deploy helper reconciles the supported single-process Fly profile
into each candidate. Unsupported settings fail before machine mutations;
extend its translation and tests before introducing additional Fly features.


The combined compatibility runtime can forward each role through
`ANARLOG_AI_ORIGIN`, `ANARLOG_SYNC_ORIGIN`, `ANARLOG_CORE_ORIGIN`, and
`ANARLOG_BILLING_ORIGIN`. Unset origins keep the existing local route behavior.
The proxy preserves uploads, response streams, and WebSocket upgrades; it does
not retry writes or follow redirects. Drain permits cover proxied requests and
upgraded connections through completion. Origins are rejected in standalone
roles to prevent routing cycles.

`api_cd.yaml` selects one explicit service per dispatch. `gateway` and `legacy`
retain the existing public URLs with forwarding profiles; deploy and verify all
standalone services before activating these profiles. The default dispatch is
`gateway`. The optional `image` input accepts only an immutable API image digest.
Keep the image and configuration together in the rollout record. Explicit drain
adoption requires independent verification that the exact image handles SIGUSR1;
health alone does not establish that capability. Secrets are filtered by the
selected runtime before staging. Core's checked-in profile enables cleanup so
routine deployments preserve worker ownership.
