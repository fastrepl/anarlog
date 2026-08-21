# Enterprise owned-stack readiness roadmap

Parent: ANLG-131.

Anarlog's enterprise position is that customers can own every sensitive part of the stack. The community application and shared contracts stay MIT. Enterprise orchestration, administration, deployment, licensing, and meeting-bot services are commercially licensed.

## v1 advocacy vs later certification

v1 is a customer-controlled data plane plus admin controls that are true in product, not marketing:

- Local-first notes on Mac and Windows
- Customer-hosted capture (Google Meet visible bot, Zoom RTMS)
- Workspace policies, SSO/SCIM, metadata-only usage analytics
- Offline license validation, no mandatory telemetry
- CLI/MCP/export as the no-lock-in surface

Later: SOC 2 / ISO 27001 / AIUC-1 / HIPAA programs, certified-cloud SKU, Teams Graph media bot in Azure, MDM remote wipe, virtual-camera disclosure.

## Workstreams

| Workstream | Tickets | v1 vs later |
| --- | --- | --- |
| Capture data plane | ANLG-223 and children | Meet + Zoom v1; Teams after Meet reliability |
| Inference routing | ANLG-132 | Policy model v1; certified-cloud providers later |
| Admin / deletion | ANLG-133, 216, 217, 218 | Policies + SSO/SCIM + analytics v1; MDM later |
| Disclosure / consent | ANLG-134, 135 | Slack huddle transport v1; virtual camera later |
| Trust / procurement | ANLG-136 | Questionnaires and DPA v1; certifications later |
| Deployment SKUs | ANLG-137, 233 | Customer-hosted data plane v1; certified cloud later |
| No lock-in | ANLG-138 | CLI/MCP/export v1; public HTTP API later |

## Platform constraints

- Mac and Windows both ship the MIT client. Linux is a cloud-agent/dev target, not a GA desktop SKU.
- Local-first SQLite remains the source of truth on device. Cloud rows are ciphertext plus metadata.
- Customer-hosted capture still needs egress to Meet/Zoom/Teams. Air-gap is "private data plane", not "join public meetings offline".
