# No-lock-in data and agent access

ANLG-138.

## Promise

Customers can inspect, export, automate, and leave with their Anarlog data. Notes are local-first SQLite. Cloud holds ciphertext plus metadata. Agents see only what the user (or workspace policy) grants.

## Durable surfaces (v1)

- Desktop/CLI against the local SQLite canonical model
- MCP tools that call the same local/session APIs
- Structured export of sessions (markdown/JSON) from the client
- Session ingest envelopes from the customer-hosted capture plane

## Later

Versioned HTTP API for automation, workspace-scoped agent tokens, and offboarding bundles that include E2EE key material the customer already holds.

## Boundaries

Local recordings and notes never become server-readable to satisfy an agent. Workspace remote deletion (ANLG-133) can drop cloud ciphertext; offline devices remain a disclosed limitation. Agent write-back is limited to the same session document schema the human editor uses.
