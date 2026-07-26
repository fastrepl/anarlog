# Mobile UX design

Lofi mockups for Anarlog mobile v1 (originals were hand-drawn wireframes, 2026-07-26, recreated here as SVG).

Linear project: https://linear.app/fastrepl-inc/project/anarlog-mobile-3ccc6cca38c1

## Home — session timeline (`home.svg`)

- Header: profile avatar (left, opens the account sheet), search (right).
- Above the Start-listening pill: "New note" and "Import memo" outline buttons — creating notes and pulling in voice memos are first-class home actions.
- Sessions grouped by day, reverse chronological: Tomorrow above Today above Yesterday.
- Within Today, upcoming sessions render above a red "now" divider (dot + full-bleed line); past sessions render below it. The upcoming card shows a relative time label ("10 mins later").
- Session cards: title, overflow ellipsis, relative time subtitle. Untitled sessions show a placeholder title.
- Pinned bottom: large black "Start listening" pill with red recording dot. Starts a new session and opens the note screen in listening mode.

## Note — session detail (`note.svg`)

- Header: back arrow (left), overflow menu (right).
- Editable title, then a free-form note body filling the screen.
- While listening, a bottom sheet sits at the bottom: a chevron handle above a red rounded panel with a live mic-driven waveform. Tapping the waveform stops listening and saves the recording as the session's audio attachment.
- Tapping the chevron expands the sheet to show recording status and duration. Live transcript is deliberately not shown — the phone sits on the table during in-person conversations; transcription happens after the recording syncs.

## Direction

- Local-first: reads and writes go to the on-device SQLite database (canonical schema from `crates/db-app`, transport via `crates/mobile-bridge`), never gated on network.
- Scope bias per ANLG-70: meeting recall, notes, summaries, transcripts, lightweight capture.
