---
name: anarlog-film
description: "Use when making a narrated motion-graphics film, explainer, or launch clip about Anarlog or one of its features (privacy, local-first capture, BYOK, MDM, open source): grounding the claims in the code, writing the script, building the deterministic HTML stage in Anarlog's Swiss-forensic style, timing it to John's voice take, rendering 16:9 and 9:16 MP4s, and checking every frame."
---

# Anarlog film

A film is one deterministic HTML stage (1920x1080, or 1080x1920) that plays as an interactive page and renders frame by frame to MP4, with one narration take laid under it untouched. Anarlog films argue one thing: **your meeting data should stay under your control.** Big Tech cloud capture is the villain; Anarlog's local-first design is the answer, shown with real paths, real settings, and real defaults.

Posting and platform specs live in the `anarlog-film-publish` skill.

## Workflow

1. **Ground every claim first.** Read the code and docs before writing a line: `docs/enterprise/managed-deployment.mdx`, `crates/db-app/src/managed_settings.rs`, `apps/desktop/src/settings/ai/` (providers and local models), `docs/security.mdx`, `LICENSE`. Record each on-screen claim and its source in the film's `script.md`. If the code does not do it today, the film does not say it (for example, self-hosted Cloud Sync is forthcoming, not shipped).
2. **Write the script before any scene.** 5-7 beats, one line each, about 38-45s spoken. Each beat gets one headline that echoes its line. See [references/narration.md](references/narration.md).
3. **Build the film** in a scratch folder outside the repo, under `~/Documents/anarlog-films/<epNN-slug>/`:
   - Copy `assets/film-template.html` there as `film.html` and replace the example scenes.
   - Write scenes in scene time with the identity `TIME_MAP`. Retime the film-level block together: `DURATION`, `CUT`, `CHAPTERS`, `SCENES`, `MARK`, and the `renderSx` calls in `render()`.
   - Recipes for each scene type are in [references/scenes.md](references/scenes.md).
4. **Check frames as you go.** `node scripts/render.mjs film.html shots shots <times>`, then `uv run --with pillow python3 scripts/sheet.py shots 3`, and read `shots/sheet.png`. Never ship a frame you have not looked at. See [references/verify.md](references/verify.md).
5. **Narrate, then time the film to the take.** One take in John's voice clone; map its line onsets with `scripts/voice-lines.py`; add `TIME_MAP` anchors so each beat lands on its words. Never cut, stretch or normalize the take. Each take costs credits: get John's OK before generating.
6. **Music.** Lay the shared Anarlog theme under the take with `scripts/music.py bed`. See [references/music.md](references/music.md).
7. **Render and mux.** `node scripts/render.mjs film.html frames frames`, then `MUSIC=music.wav scripts/mux.sh frames film.mp4 narration.mp3 <NARRATION_AT>`.
8. **Vertical cut** for Reels, Shorts and TikTok, rendered natively at 1080x1920, never cropped: `scripts/captions.py` writes the SRT from the voice-only MP4, `scripts/vertical-page.py` flips the page's `FORMAT` and draws the captions, then render and mux it the same way.

## Visual language

Swiss-technical, Bloomberg-terminal, forensic dossier. Nothing from Char's films: no dithered meadow, no serif display face, no rounded bubbles, no hand-drawn red marks.

- **Two polarities on one grid.** Every frame is either:
  - **Red zone (third-party cloud):** obsidian `#121210`, hairline grid `#272623`, newsprint ink, crimson `#dc2626` for warnings, packets and tags. The egress counter in the top rail climbs.
  - **Local zone (Anarlog):** newsprint `#f2f1ef`, white `#ffffff` panels with 1px `#302d2a` borders, `--grey-900` ink, brand-yellow `oklch(0.9484 0.0672 90.6)` highlight sweeps, emerald `#15803d` for the verified local boundary. The rail reads `NET EGRESS 0 B`.
  - Switch zones with the polarity wipe (`CUT`): newsprint sweeps across behind a 2px emerald scan line, then every token flips at once. One cut per film, from problem to answer. Never fade between zones.
- **Geometry.** `border-radius: 0` on everything: panels, tags, nodes, captions, buttons. 1px borders. Orthogonal wires only (no curves, no diagonals).
- **Type.**
  - Headlines: `system-ui` bold, 88px, `letter-spacing: -0.035em`, 1.04 leading, one per scene, in the left column (x=120 to `--demo-x` minus 80). Wrap each sentence in a `<span>` so lines break between sentences, never inside one.
  - Everything technical is `ui-monospace` (SF Mono): tags, node labels, paths, JSON, timestamps, rails. Mono labels are uppercase with 0.06-0.1em tracking.
  - One mono tag above each headline (`■ THIRD-PARTY CLOUD`, `■ ANARLOG · ON DEVICE`) and at most one mono footnote under it. No body copy: the voice carries the second sentence.
  - Minimum sizes at 1920x1080: 17px for mono micro-labels, 22px for any value a viewer must read, 26-28px for primary demo text.
- **Layout.** Headline left, demo right (`.demo` at `--demo-x`, default 920px, 880px wide, vertically centred between the rails). Content stays inside the rails (top 72px, bottom 56px) and 120px from the side edges.
- **Telemetry rails** are always on (except the end card): top rail with the docked wordmark, `EP NN`, the chapter, the scene counter `02 / 06` and the egress readout; bottom rail with the timecode, the zone label and `anarlog.so`.
- **Wordmark.** The exact paths of `apps/web/public/logo.svg` (viewBox `0 0 1205 334`), inline, `fill="currentColor"` so it follows the zone. One continuous motion: it wipes in centred on the grid behind a scan line, docks into the top rail (16:9) or the bottom centre (9:16), and returns to centre for the end card. Never cut it out and bring a new one in.
- **End card.** Newsprint, centred wordmark, `Your meetings stay on your machine.` (bold sans), `anarlog.so` (mono). The rails fade out.
- **Motion.** Mechanical and fast: hairline wipes left to right (0.25-0.4s), hard cuts for tags and stamps, `E.snap` easing, no overshoot, no bounce, no blur, no rotation. Packets move at constant speed along wires. Counters tick, never animate smoothly between unrelated values.

## Narrative discipline

- **Big Tech is the villain, as a system, not a person or a named company.** Show the architecture (bots, vendor clouds, sub-processors, US jurisdiction, opaque retention) and label it `ILLUSTRATIVE: TYPICAL CLOUD NOTETAKER`. Never name or depict a real competitor, executive, or logo.
- **Every Anarlog claim is verifiable**: a path, a setting key, a provider, a license. Prefer showing the real artifact (`~/Library/Application Support/anarlog/app.db`, `managed_settings.json`, `github.com/fastrepl/anarlog`) over adjectives.
- **Do not overclaim.** Anarlog's optional cloud features (Cloud Sync, shared notes, managed AI) run in the US; Cloud Sync is end-to-end encrypted. No certification claims (no SOC 2, ISO 27001, HIPAA). No self-hosted sync claims until it ships. List these in each episode's "must not say" section.
- **Data privacy, security, sovereignty** are the three themes. One episode argues one point.

## Gotchas

- Everything on screen is a pure function of scene time `t`: no `Math.random`, `Date`, `performance.now()` inside a render function, no CSS animations or transitions, no timers. Fake live signals (waveforms, counters, REC clocks, blinking dots) with seeded functions of `t` (`noise(i, t)`, `egressMB(t)`). `requestAnimationFrame` only advances the playback clock; `render(T)` draws every frame.
- Typed text must not reflow: render the untyped remainder invisibly (see S1) and give the caret a negative right margin.
- Elements that wipe in set `visibility: hidden` at zero, so a hidden panel never catches a stray hairline.
- Pin growing containers to a fixed `top` so new rows do not push earlier content.
- `render.mjs` exits non-zero on any page error or console error. Fix every one before rendering frames.
- Playwright: the repo does not depend on it. `render.mjs` tries `PLAYWRIGHT_MODULE`, then the repo's `node_modules`, then `~/Dev/char/storybook`. Never add it to `package.json` for a film.
- On this Mac, Homebrew's `node` can break; run the scripts with mise Node 22 (`~/.local/share/mise/installs/node/22.*/bin/node`).
- Keep films, renders, takes and music outside the repo. Only the skill is committed.

## References

- [references/scenes.md](references/scenes.md): recipes for the egress map, local SQLite inspector, zero-bot capture timeline, `managed_settings.json` policy lock, BYOK/EU router, and polarity split-screen.
- [references/narration.md](references/narration.md): script rules, John's ElevenLabs voice and its v4 editor gotcha, mapping the take, building `TIME_MAP`.
- [references/music.md](references/music.md): the Anarlog theme, the Suno prompt, cutting and laying the parts.
- [references/verify.md](references/verify.md): commands, review checklist, sync check, delivery.
- [references/sume-parody.md](references/sume-parody.md): AI skits and recurring parody series on sume.com, with credit caps and disclosure rules.
