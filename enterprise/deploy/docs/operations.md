# Capture data plane operations

This is the customer-hosted meeting capture distribution. Evaluation Compose does not require an offline license. Production Compose and Helm do.

## Layout

| Mode               | Manifest                                 | License  | Typical use                          |
| ------------------ | ---------------------------------------- | -------- | ------------------------------------ |
| Evaluation         | `enterprise/control-plane/compose.yaml`  | optional | Single-box soak of the control plane |
| Production Compose | `enterprise/deploy/compose.prod.yaml`    | required | Compose on customer VMs              |
| Production Helm    | `enterprise/deploy/helm/anarlog-capture` | required | Kubernetes                           |

Pinned images live in the Compose files and Helm `values.yaml`. Do not float untagged `latest`.

## Secrets

Customer-managed, never committed:

- `ANARLOG_ENTERPRISE_WORKSPACE_TOKENS` / Helm `workspaceTokens.existingSecret`
- `ANARLOG_ENTERPRISE_LICENSE` + `ANARLOG_ENTERPRISE_LICENSE_KEY`
- Postgres and object-storage passwords
- Optional Zoom client/webhook secrets
- Optional customer STT URL (`ANARLOG_ENTERPRISE_STT_URL`)

Offline license validation uses HMAC-SHA256 over versioned claims. Telemetry is not exported; scrape `/health/live` and `/health/ready` from customer-owned collectors.

## Health, backups, upgrades

- Control plane liveness/readiness: `/health/live`, `/health/ready`.
- Back up Postgres volume `postgres-data` (durable jobs, leases, scheduled captures), object storage `object-data`, and the Meet worker recordings PVC separately.
- Schema migrations are additive. Roll forward by deploying the new image; roll back by redeploying the previous image so long as no `-- breaking` migration was applied.
- Capture workers are fenced by durable leases. A crashed replica does not strand a job: another worker can reclaim after lease expiry.

## Capture workers

Google Meet workers join as a visible Anarlog participant. If `ANARLOG_ENTERPRISE_CAPTURE_JOB_ID` is unset they poll dispatched calendar jobs for the workspace. Zoom uses RTMS in the control plane (no browser bot). Optional STT is customer-hosted; leaving `ANARLOG_ENTERPRISE_STT_URL` empty keeps transcript finalization local to whatever the worker is configured with.

## Network

The data plane must reach the meeting platform (Meet, Zoom, or Teams). A fully air-gapped cluster cannot join public cloud meetings. Outbound policy should allow only the meeting provider, customer STT, and object storage.

## Stock client

Point the stock Anarlog desktop/web client at the customer control plane workspace token and session ingest endpoints. No Fastrepl-operated capture service is required after this package is running.
