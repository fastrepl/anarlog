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

Role selection does not change Fly routing or provision applications. The
existing deployment stays on `all` until service-specific configuration,
readiness, public routing, and deployment/rollback continuity are verified.
Keep existing URLs working while clients and webhook providers migrate.
