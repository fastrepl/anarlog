# Anarlog Enterprise control plane

This commercially licensed service packages the workspace-scoped enterprise capture API with automatic PostgreSQL migrations, fail-closed startup configuration, and graceful shutdown. It durably appends provider-neutral capture events, projects each revision into the shared session-ingest contract, and delivers those revisions to authorized clients.

## Evaluation deployment

Docker with Compose v2 is required. From the repository root:

```sh
cp enterprise/control-plane/.env.sample enterprise/control-plane/.env
```

Replace both instances of the database password with the same random hexadecimal value, and replace the workspace bearer token with at least 32 random characters. Hexadecimal values avoid URL-encoding differences between `POSTGRES_PASSWORD` and `ANARLOG_ENTERPRISE_DATABASE_URL`.

Start the service:

```sh
docker compose \
  --file enterprise/control-plane/compose.yaml \
  --env-file enterprise/control-plane/.env \
  up --build --wait --detach
```

The bundle binds the API to `127.0.0.1:8080` by default and does not publish PostgreSQL. Verify it with:

```sh
curl --fail http://127.0.0.1:8080/health/ready
curl --fail \
  --header 'Authorization: Bearer YOUR_WORKSPACE_TOKEN' \
  'http://127.0.0.1:8080/v1/workspaces/evaluation-workspace/session-envelopes?consumerId=evaluation-device&after=0'
```

Stop it without deleting database data:

```sh
docker compose \
  --file enterprise/control-plane/compose.yaml \
  --env-file enterprise/control-plane/.env \
  down
```

## Infisical

The deployment accepts every secret through environment variables, so Infisical can inject them without generating a `.env` file. Store the variables from `.env.sample` in one Infisical environment, then run:

```sh
infisical run --env=prod -- \
  docker compose \
    --file enterprise/control-plane/compose.yaml \
    up --build --wait --detach
```

See the official [`infisical run` documentation](https://infisical.com/docs/cli/commands/run) for machine identity and project options. Do not use `--watch` for a production Compose process; perform a controlled restart when credentials rotate.

## Configuration

| Variable | Required | Purpose |
| --- | --- | --- |
| `ANARLOG_ENTERPRISE_DATABASE_URL` | Yes | PostgreSQL connection URL. Startup fails if the database is unavailable or migrations fail. |
| `ANARLOG_ENTERPRISE_WORKSPACE_TOKENS` | Yes | JSON object mapping workspace IDs to bearer tokens. At least one unique token of 32–512 bytes is required. |
| `ANARLOG_ENTERPRISE_BIND_ADDRESS` | No | Listener address. Defaults to `0.0.0.0:8080`; the image sets the same value. |
| `ANARLOG_ENTERPRISE_DATABASE_MAX_CONNECTIONS` | No | PostgreSQL pool size from 1–100. Defaults to `10`. |
| `RUST_LOG` | No | Standard tracing filter. Defaults to request and service information. No telemetry is exported. |

`GET /health/live` is process-only liveness. `GET /health/ready` checks PostgreSQL. Capture and delivery routes require `Authorization: Bearer <token>` and reject a token used against any workspace other than its configured workspace.

Capture workers create a durable job with `POST /v1/workspaces/{workspace_id}/capture-jobs/{job_id}`, claim it through `/claim`, and renew the returned 60-second fencing lease through `/lease`. Every new event appended to `/events` must carry that lease identity. An expired lease can be reclaimed with a higher epoch, which prevents the prior worker from advancing the job; an identical already-persisted event remains safe to replay. Event IDs and zero-based sequences are idempotency keys. Each accepted event atomically advances the PostgreSQL checkpoint and publishes a delivery revision, while conflicting IDs, sequences, lifecycle transitions, or stale leases fail closed.

The static token map is intended for the evaluation bundle and sits behind the `WorkspaceAuthenticator` interface. Production OIDC, SCIM, offline license enforcement, object storage, and meeting browser workers are outside this service.

The runtime image uses pinned Debian and Rust base-image digests, runs as UID/GID `10001`, has a read-only root filesystem in Compose, and persists only PostgreSQL data. Terminate TLS at a trusted reverse proxy before exposing the API beyond localhost.
