# Scene recipes

Each scene is one `.layer` with a `.copy` (tag, headline, optional footnote) and a `.demo` holding an 880px-wide `.dz` box, plus a `renderSx(t)` that takes scene time. The template already holds the CSS and helpers named here: `P`, `E.snap`, `win`, `show`, `move`, `wipe`, `typed`, `draw`, `noise`, `along`, `pointsOf`, `renderCopy`, `clock`, and the components `.node`, `.panel`, `.row`, `.stamp`, `.tag`, `.hl`, `.wave`, `.tbl`, `.boundary`, `.route`, `.group-label`, `.code-line`, `.repo`.

Skeleton:

```html
<div class="layer" id="s4">
  <div class="abs copy">
    <span class="tag hot"><span class="dot"></span>Anarlog · on device</span>
    <h2><span>No bot.</span> <span>Just your machine.</span></h2>
    <div class="note">Mic + system audio · captured locally</div>
  </div>
  <div class="abs demo"><div class="dz"><!-- the demo --></div></div>
</div>
```

```js
const s4 = $("s4");
function renderS4(t) {
  show(s4, win(t, 20.0, 28.3)); // on screen 20.0-28.3, hard 0.18s cuts
  renderCopy(s4, t, 20.0); // tag, headline, footnote wipe in from +0.25s
  const p = P(t, 20.4, 0.3);
  show($("s4Cap"), p > 0 ? 1 : 0);
  wipe($("s4Cap"), p); // hairline wipe, left to right
}
// and call renderS4(t) from render()
```

`.tag.hot` takes the zone accent: crimson in the red zone, emerald in the local zone. Use one per scene.

## Opening line

A centred bold line types out on the obsidian grid after the wordmark docks (template `S1`). Keep it under ~45 characters; it may wrap to two lines at 104px. The untyped remainder is rendered invisibly so the line never reflows; keep that pattern when you change it.

## Forensic egress map (red zone)

The template's `S2`. A call node whose last participant tile flips to `BOT ●`, an orthogonal wire to a `VENDOR CLOUD · US` node, then a bus fanning out to four `SUB-PROC 0N` nodes. Crimson packets ride every wire.

- Wires are `<polyline class="wire hot" points="x,y x,y ..." pathLength="1">` inside an SVG the size of `.dz`. Draw them with `draw(wire, P(t, at, 0.4))`.
- Packets are 12px squares placed with `along(pointsOf(wire), u)` where `u = ((t - start) * speed + k / n) % 1`: constant speed, evenly spaced, deterministic.
- Keep nodes at least 24px apart and wires on whole-pixel coordinates that hit node centres.
- The egress counter in the top rail starts with the first packet (`EGRESS_FROM`).
- Label the scene `ILLUSTRATIVE: TYPICAL CLOUD NOTETAKER` in the footnote. Never draw a real product's UI or logo.

## Custody record (red zone)

A `.panel` of `.row`s (key in mono uppercase, value typed with `typed(text, t, at, 44)`), each value landing on the phrase that says it. While a row is typing, give it `background: var(--crimson-wash)`. End with a `.stamp` (3px crimson border, mono, `OUT OF YOUR CUSTODY`) that cuts in with a 1.12 → 1 scale over 0.14s, straddling the panel's bottom edge.

```html
<div class="panel" id="s3Panel">
  <div class="panel-head"><span>CUSTODY RECORD</span><span>meeting_2026-11-26.m4a</span></div>
  <div class="row"><span class="k">Jurisdiction</span><span class="v hot"></span></div>
</div>
<div class="stamp" id="s3Stamp" style="right: 28px; bottom: -36px">OUT OF YOUR CUSTODY</div>
```

## Polarity cut

`CUT = { at, dur }` in the timing block. `renderBackdrop` grows `#wipe` (newsprint, 2px emerald leading edge) across the stage, then sets `stage.dataset.zone = "local"` and every token flips. The last red scene stays visible until the wipe has covered it (`win(t, a, CUT.at + CUT.dur, 0.18, 0.01)`); the first local scene cuts in as it lands. One cut per film.

## Zero-bot capture timeline (local zone)

The template's `S3` top panel: a `.panel-head` with a blinking `● REC hh:mm:ss` clock (from `clock(base + t - a)`) and `BOTS IN CALL: 0` in emerald, then two `.wave-row`s (`MIC`, `SYSTEM AUDIO`) of 44 bars each. Bar heights come from `noise(i, t)` times a slow envelope per row, so the two sources move independently and every render matches.

## Local SQLite inspector (local zone)

The template's `S3` lower panel: the real path in the head, then a `.tbl` with `SESSIONS / LENGTH / STORED` and three rows whose `STORED` column reads `LOCAL` in emerald. The current meeting's row title gets the yellow `.hl` sweep (`--hl` from 0 to 100%) and its length ticks with the REC clock. Close the scene with the `.boundary`: a 2px emerald frame around both panels with a `DEVICE BOUNDARY · NET EGRESS 0 B` tab.

Use the real paths: macOS `~/Library/Application Support/anarlog/app.db`, Windows `%APPDATA%\anarlog\app.db`, Linux `~/.local/share/anarlog/app.db`. Show table names only if they match `crates/db-app` (`sessions` does).

## BYOK / EU gateway router (local zone)

An input node (`Audio + transcript`) on the left, a vertical trunk at x≈290, and two groups of `.route` rows on the right under `.group-label`s: `ON THIS DEVICE` (Parakeet, Whisper, Ollama) and `YOUR OWN ENDPOINT` (Mistral EU, Azure OpenAI EU region, LiteLLM / Bifrost gateway).

```js
// [scene time, route index]: the active route follows the words that name it.
const PICKS = [[29.5, 0], [30.3, 1], [31.2, 2], [32.6, 3]];
const pick = PICKS.filter(([at]) => t >= at).pop();
routes.forEach((r, i) => r.classList.toggle("on", pick?.[1] === i));
wires.forEach((w, i) => w.setAttribute("class", pick?.[1] === i ? "wire ok" : "wire"));
```

The active row also gets a yellow sweep behind its label. Only list providers the settings UI actually offers (`apps/desktop/src/settings/ai/`).

## managed_settings.json policy lock (local zone)

A `.panel` headed `managed_settings.json` / `MDM · SCHEMA 1`, with `.code-line`s (line number, source, lock cell). Type the JSON fast (90 chars/s) so it is complete before the voice says "lock"; then, key by key on the word, a `LOCKED` badge wipes in and a yellow sweep runs under the line. Close with the `.repo` bar (`github.com/fastrepl/anarlog` + `MIT`).

Only use keys `crates/db-app/src/managed_settings.rs` accepts, with their allowed values: `cloud_sync_enabled: false`, `telemetry_consent: false`, `crash_reporting_consent: false`, `automatic_updates: false`, `intelligence_disabled: true`, `save_recordings`, `auto_start_scheduled_meetings`, `audio_retention: "forever"`, `current_stt_provider: "custom"` with `stt:custom`. The policy file never contains an API key.

## Polarity split-screen

For a direct comparison in one frame: two `.layer`s clipped to halves, each forced to its own zone.

```html
<div class="layer" id="s5Red" data-zone="red" style="clip-path: inset(0 50% 0 0); background: var(--obsidian)"></div>
<div class="layer" id="s5Local" data-zone="local" style="clip-path: inset(0 0 0 50%); background: var(--page)"></div>
```

The zone rules match any element with `data-zone`, so each half carries its own tokens while the stage stays in one zone. Put a 2px divider at x=960 in `--line-strong`, one mono header per half (`THIRD-PARTY CLOUD` / `ANARLOG`), and mirror the same rows on both sides (`AUDIO`, `TRANSCRIPT`, `RETENTION`, `JURISDICTION`) so the eye compares line by line. No headline column in this scene: put a single centred headline above both halves. In 9:16, stack the halves top and bottom instead.

## End card

Under the returning wordmark (`MARK.endIn`): the promise line wipes in at +0.7s, `anarlog.so` follows at +1.4s, and the rails fade. Hold it until the music's ending has rung out.

## 9:16

The template's vertical rules (`.stage[data-format="9:16"]`) already move the copy to the top (headline 76px, footnote hidden), the demo box to x=100, y=560..1500, the mark to the bottom centre and the captions to y=1560. A scene that needs more adds its own `.stage[data-format="9:16"] #sN ...` rules in the film's CSS. Keep every demo 880px wide so it fits both formats unchanged.
