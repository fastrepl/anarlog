# Mobile UX design

Lofi mockups for Anarlog mobile v1 (originals were hand-drawn wireframes, 2026-07-26, recreated here as SVG).

Linear project: https://linear.app/fastrepl-inc/project/anarlog-mobile-3ccc6cca38c1

## Home — session timeline (`home.svg`)

- Header: profile avatar (left), search (right).
- Sessions grouped by day, reverse chronological: Tomorrow above Today above Yesterday.
- Within Today, upcoming sessions render above a red "now" divider (dot + full-bleed line); past sessions render below it. The upcoming card shows a relative time label ("10 mins later").
- Session cards: title, overflow ellipsis, relative time subtitle. Untitled sessions show a placeholder title.
- Pinned bottom: large black "Start listening" pill with red recording dot. Starts a new session and opens the note screen in listening mode.

## Note — session detail (`note.svg`)

- Header: back arrow (left), overflow menu (right).
- Editable title, then a free-form note body filling the screen.
- While listening, a red rounded panel with a live waveform sits at the bottom. Tapping it stops listening.

## Direction

- Local-first: reads and writes go to the on-device SQLite database (canonical schema from `crates/db-app`, transport via `crates/mobile-bridge`), never gated on network.
- Scope bias per ANLG-70: meeting recall, notes, summaries, transcripts, lightweight capture.
