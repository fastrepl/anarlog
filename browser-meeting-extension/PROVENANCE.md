# Provenance: Meet extension

Upstream: Char, Fastrepl, Inc. Frozen export of commit `d4608723d8ac328ab91091803a08c4585a0dc244`, which contains the reviewed Meet attachment head `12bf21bcbd3ceb763a9f3495b803135574037ec0`. The export's `LICENSE` is GPL-3.0-or-later. On 2026-11-19, Fastrepl authorized distribution of its Char code ported here under Anarlog's MIT license.

| Anarlog file | Char source (`char/browser-save-extension/`) | Use |
| --- | --- | --- |
| `extension/meeting-content.js`, `meeting-speakers.js`, `meeting-chat.js`, `meeting-camera.js` | `next/` files of the same names | Copied. Renamed to Anarlog globals, message types and storage keys, and the copy reworded. Behavior is unchanged. |
| `extension/meeting-background.js` | `next/meeting-background.js` | Modified. Anarlog's host name now comes from `config.js`. Added: a timeout for a host that never sends `connected` (an old host), the recorder's `pausable` capability, and `refused` answers that keep the link. Only Stop is a menu item. |
| `extension/toolbar.js` | `next/toolbar.js` | Reduced to the Meet click and the Stop menu. Every save-page feature was removed. |
| `extension/manifest.json`, `config.js`, `background.js` | `extension/manifest.json`, `src/next.ts` | Re-authored. The permissions follow Char's Meet build (`activeTab`, `scripting`, `contextMenus`, `nativeMessaging`, no host permissions). There are no content scripts, and no Char key, ID, name or icons. |
| `test/*.test.ts` | `src/meeting-*.test.ts`, `src/toolbar.test.ts` | Page-script tests copied with renamed identifiers. The background and toolbar tests were adapted to the changes above. |

Icons are Anarlog's stable desktop icons (`apps/desktop/src-tauri/icons/stable/32x32.png`, `128x128.png`).
