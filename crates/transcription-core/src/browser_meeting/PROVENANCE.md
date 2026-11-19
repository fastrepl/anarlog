# Provenance: browser Meet bridge

Upstream: Char, Fastrepl, Inc. Frozen export of commit `d4608723d8ac328ab91091803a08c4585a0dc244`, which contains the reviewed Meet attachment head `12bf21bcbd3ceb763a9f3495b803135574037ec0`. The export's `LICENSE` is GPL-3.0-or-later. On 2026-11-19, Fastrepl authorized distribution of its Char code ported here under Anarlog's MIT license.

| Anarlog file | Char source | Use |
| --- | --- | --- |
| `route.rs` | `char/apps/desktop/src/main/online/browser-meeting.ts` | Translated to Rust and modified. Kept: holds, link staleness and takeover, owner-aware detach, the follow checks (confirmed call, browser family, exact canonical URL), notice/camera handout deduplication, outcome acceptance, and refusal wording. Link tokens now come from the shared meeting runtime. Starts go through the app's recorder port. The follow checks also apply to takeovers. A claim nobody received is released. |
| `relay.rs` | `char/apps/cli/src/next/browser-meeting.ts` | Translated and modified. Kept: one-change-at-a-time reporting, first-`started` handout, polling, and close/leave semantics. It runs in the app per socket connection rather than in the CLI. Cues go to the route as they arrive, and refused commands keep the link. |
| `call.rs` | `meetCallOf`, `browserOf` and `BROWSERS` in `browser-meeting.ts` | Translated, with the same accepted and refused forms. |
| `outcome.rs` | `CHAT_OUTCOMES`, `CHAT_UNSENT` and `CAMERA_UNTOUCHED` in `browser-meeting.ts`; outcome names from `char/browser-save-extension/next/meeting-chat.js` and `meeting-camera.js` | Translated. Mapped onto the shared four notice results. |
| `cues.rs` | Event shapes from `char/browser-save-extension/next/meeting-speakers.js` | Reference only. The reader is new. |
| `protocol.rs`, `port.rs`, `transport.rs` | None | New for Anarlog: the socket, hello, endpoint record, and port replace Char's loopback HTTP route and CLI. |
| `tests.rs` | `char/apps/desktop/src/main/online/browser-meeting.test.ts` | The scenarios were re-expressed against Anarlog's port and runtime rules. |
| `crates/chrome-native-host/src/lib.rs` | None | New. Char's host was the `char` CLI; nothing was reused from its paths, host identity, or account secrets. |
