# Microsoft Teams enterprise capture

## Selected connector

Anarlog does **not** use a Chromium bot against the Teams web client. That path is unsupported against current Teams terms and DOM churn.

The supported connector is a **Microsoft Graph application-hosted media bot** running as a Windows sidecar. The sidecar speaks the MIT `MeetingSdkBridge` protocol (JSON lines) to `anarlog-enterprise-meeting-sdk-bridge-worker`, which normalizes events onto the shared capture contract (`CaptureProviderKind::MicrosoftGraph`, `MeetingPlatform::MicrosoftTeams`).

## Official alternative and constraints

Graph application-hosted media requires:

- Azure subscription and a Teams app with application-hosted media
- Windows Server for the media bot runtime
- Certificate/app registration, organizer policy, and lobby admission under tenant admin control

This is **not** deployable on Linux-only clusters or a fully air-gapped network that cannot reach Teams/Graph.

## Reliability matrix

Replay fixtures in `enterprise/meeting-sdk-bridge-worker/tests/teams_reliability.rs` cover:

| Scenario                              | Terminal reason              |
| ------------------------------------- | ---------------------------- |
| Admitted then capturing               | (non-terminal)               |
| Lobby timeout                         | `admission_timeout`          |
| Organizer denied                      | `admission_denied`           |
| Removed by organizer                  | `removed_from_meeting`       |
| Meeting ended                         | `meeting_ended`              |
| Participant upsert / leave / captions | non-terminal until host ends |

## Deployment modes

| Mode                                       | Supported           |
| ------------------------------------------ | ------------------- |
| Customer Azure + Windows Server sidecar    | yes                 |
| Linux-only Helm capture chart              | no (Meet/Zoom only) |
| Air-gapped / no Graph                      | no                  |
| Browser worker against teams.microsoft.com | no                  |
