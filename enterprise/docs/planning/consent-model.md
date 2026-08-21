# Meeting recording disclosure and consent model

Parent: ANLG-135.

Posting a recording/transcription disclosure is an opt-in transport. A sent disclosure is not proof that every participant consented. Anarlog keeps disclosure delivery and consent evidence as separate product concepts.

## V1 Slack huddle transport

- `consent_auto_send_chat` defaults to false.
- After listening starts, Anarlog may post one disclosure to the recognized native Slack Huddle chat (macOS Accessibility only).
- Delivery is once-per-session, with bounded retry/cancellation. Posting failure never stops listening.
- The disclosure is excluded from captured Memos.
- Safe chat mutation stays disabled for Zoom, Meet, Teams, Webex, and browser surfaces until each has controlled live validation.

## Evidence vs consent

| Event | Product meaning |
| --- | --- |
| `session_disclosure_attempts` row with `delivery = sent` | Transport evidence only |
| `session_participant_consent.status = unknown` | Default, including late joiners |
| `status = consented` with `source` in `explicit_chat_reply` / `explicit_ui` | That participant answered; not legal consent for the room |
| `status = declined` | Stop listening. Still not a legal record of anyone else's choice |

`sessionHasLegalConsent()` is always false. Schema CHECK constraints reject a `disclosure_sent` consent source. These tables are local-only and are not in CloudSync or the E2EE domain.

## Decline, late joiners, unseen chat

- Anyone declining stops listening.
- Late joiners start `unknown` / `unseen`.
- Participants who cannot see huddle chat remain `unknown`.
- Regional defaults, DPA language, and whether chat replies are sufficient evidence are counsel questions, not product defaults.

## Enterprise controls

Workspace admins can require or forbid auto-posting later via policy. They must not get an "everyone consented" dashboard derived from disclosure delivery.

## Live verification

Slack huddle AX send is macOS-only (`send_meeting_chat_message`). Linux/cloud agents cannot complete controlled live huddle testing. Unit and fixture coverage lives in `crates/detect` and `apps/desktop/src/stt/meeting-consent.test.ts`.
