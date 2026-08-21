# Zoom RTMS enterprise capture

Anarlog captures Zoom through [Realtime Media Streams](https://developers.zoom.us/docs/rtms/), not a browser bot. The meeting materializes as an Anarlog session from RTMS audio, transcript, chat, and participant events.

## Customer tenant prerequisites

1. Zoom account on a plan that includes **Zoom Developer Pack** / RTMS.
2. A Zoom app with RTMS scopes installed to the customer tenant (account-level).
3. Host or admin approval for the Anarlog app, plus any required recording disclosure in the Zoom admin console.
4. Webhook endpoint on the Anarlog control plane (`POST /webhooks/zoom`) with `ANARLOG_ENTERPRISE_ZOOM_WEBHOOK_SECRET`.
5. Control-plane env:
   - `ANARLOG_ENTERPRISE_ZOOM_CLIENT_ID`
   - `ANARLOG_ENTERPRISE_ZOOM_CLIENT_SECRET`
   - `ANARLOG_ENTERPRISE_ZOOM_ACCOUNT_WORKSPACES` mapping Zoom account IDs to Anarlog workspace IDs

## Runtime

The control plane verifies Zoom webhooks, creates a durable capture job, and hands the RTMS session to `anarlog-enterprise-zoom-rtms-worker`. Stream reconnect, terminal reasons, and meeting finalization follow the shared capture contract. There is no visible third-party browser participant.

## Limits

- RTMS cannot join meetings the Zoom account is not authorized to record.
- Air-gapped deployments cannot reach Zoom's cloud media edge.
- Video frames are metadata-only in v1; audio + transcript + chat + participants are the durable session.
