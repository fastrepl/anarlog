# Char meeting automation (chat notice, shared Accessibility helpers)

| Field     | Value                                                                                                                         |
| --------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Upstream  | Char (`fastrepl/char`), read from a frozen export, not a checkout                                                             |
| Revision  | `d4608723d8ac328ab91091803a08c4585a0dc244`, which contains the reviewed notices branch head `729e5ed21f6cad56292b7c5bcee64d571dfc777d` |
| License   | GNU General Public License v3.0 or later (upstream `LICENSE`)                                                                 |
| Copyright | "Char Copyright (C) 2023-present Fastrepl, Inc."                                                                              |
| How used  | Source copied and modified (Anarlog crate versions, names, and exact-process targeting); see Files                            |
| Status    | On 2026-11-19, Fastrepl authorized distribution of its Char code ported here under Anarlog's MIT license. |

## Files

| Anarlog path                                     | Upstream path (`crates/desktop-agent-automation/src/`)                     | Changes                                                                                                                      |
| ------------------------------------------------ | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `src/meeting_automation/mod.rs`                  | `meeting_chat/mod.rs` (`MeetingApp`)                                       | Renamed `NativeMeetingApp`; adds `from_bundle_id`, `as_str`                                                                  |
| `src/meeting_automation/matching.rs`             | `meeting_chat/matching.rs` (call window and markers)                       | Split out so the camera switch shares it                                                                                     |
| `src/meeting_automation/macos/applications.rs`   | `common/macos/applications.rs`                                             | `lock_meeting_app`; `bundle_id_of` moved here; no enumeration of running copies; uses `anlg-overlay-kit`                      |
| `src/meeting_automation/macos/ax.rs`             | `common/macos/ax.rs`                                                       | Attribute names inlined (no `hypr-ax`); unused geometry helpers dropped                                                      |
| `src/meeting_chat/mod.rs`                        | `meeting_chat/mod.rs`                                                      | Requires the confirmed pid (no scan of every running copy); `validate_notice`; `may_have_sent`                               |
| `src/meeting_chat/matching.rs`                   | `meeting_chat/matching.rs` (chat input, open-chat control, draft)          | Unchanged rules and fixtures                                                                                                 |
| `src/meeting_chat/send.rs`                       | `meeting_chat/send.rs`                                                     | `is_focused_at_front` split into `can_type`/`can_send`; adds a test that a cancel after Return cannot make it `Cancelled`     |
| `src/meeting_chat/macos.rs`                      | `meeting_chat/macos.rs`                                                    | Exact pid only, re-checks the pid's bundle under the lock; uses `anlg-overlay-kit` input, pasteboard and main-thread helpers |

The durable claim, cancellation registry and dispatch rules in `plugins/detect/src/meeting_notice` and `crates/db-app/src/meeting_notice_ops.rs` follow the behavior of Char's `char/apps/desktop/src/main/reactions/meeting-chat-notice.ts`, `native-meeting/chat-notice-ledger.ts`, `native-meeting/chat-notice-host.ts` and `crates/desktop-napi/src/meeting_chat.rs`. They are rewritten in Rust for Anarlog's SQLite store and lifecycle, not copied.
