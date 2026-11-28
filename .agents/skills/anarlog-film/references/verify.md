# Verify and deliver

## Commands

Run from the film folder (`~/Documents/anarlog-films/<epNN-slug>/`), with `SK` set to the skill's absolute path and a working Node 22:

```sh
SK=/Users/johnjeong/Dev/anarlog/.agents/skills/anarlog-film
NODE=$(ls -d ~/.local/share/mise/installs/node/22.*/bin/node | tail -1)   # Homebrew node can be broken here

$NODE $SK/scripts/render.mjs film.html shots shots 0.6,2.6,5.0,8.4,...   # frames at film times (no list: every 2s)
uv run --with pillow python3 $SK/scripts/sheet.py shots 3               # shots/sheet.png: read it
uv run --with pillow python3 $SK/scripts/sheet.py shots 2 920,72,1800,1024  # same, cropped to the demo at full size
$NODE $SK/scripts/render.mjs film.html frames frames 30                 # every frame (run in the background)
bash $SK/scripts/mux.sh frames film-voice.mp4 narration-1.mp3 2.2       # voice only, take untouched
python3 $SK/scripts/music.py bed music 2.2 <endAt> music.wav            # prints film_end
MUSIC=music.wav bash $SK/scripts/mux.sh frames film.mp4 narration-1.mp3 2.2
```

Vertical:

```sh
uv run --with faster-whisper python3 $SK/scripts/captions.py film-voice.mp4 film.srt narration.txt
python3 $SK/scripts/vertical-page.py film.html film-vertical.html --srt film.srt
$NODE $SK/scripts/render.mjs film-vertical.html shots shots-vertical <times>
uv run --with pillow python3 $SK/scripts/sheet.py shots-vertical 8
$NODE $SK/scripts/render.mjs film-vertical.html frames frames-vertical
MUSIC=music.wav bash $SK/scripts/mux.sh frames-vertical film-vertical.mp4 narration-1.mp3 2.2
```

Before a take exists, smoke-test the vertical pipeline with a hand-written `stub.srt` (one short cue per phrase on the identity timeline); never spend credits to test layout.

- `render.mjs` sizes the viewport from the page (`FILM_SIZE=1080x1920` overrides it) and exits non-zero with `page errors:` on any page or console error. Fix every one before rendering frames.
- A frame render reads the page when it starts; if you edit `film.html` mid-render, kill it and start again.

## Shot list

Render at least: the wordmark mid-wipe (~0.6s), the opening line mid-type and complete, every scene's start, middle and end (headline in, each beat, full state), the polarity wipe mid-sweep, the first local frame, and the end card before and after the line lands. After narration, add one shot per `TIME_MAP` anchor.

## Review checklist

Read every tile of the sheet, then crop to the demo for anything small.

- **Brand.** Square corners everywhere; 1px borders; obsidian/crimson before the cut, newsprint/white/emerald/yellow after; no gradients, shadows, blur, serif type or rounded shapes. Nothing resembles Char's films.
- **Wordmark.** Proportions intact (1205x334), crisp in the rail at 30px tall, one continuous motion from centre to dock to end card, never overlapping rail text.
- **Copy.** One tag + one headline per scene, the headline echoing the voice; lines break between sentences; no straight-quote or dash violations; nothing typed reflows.
- **Layout.** Nothing clips, overflows its panel, overlaps unintentionally, or comes within 120px of the side edges; content stays between the rails. Waveforms, tables and code lines fit their panels at the widest state.
- **Truth.** Every path, key, provider and license on screen matches the code and the claim sources in `script.md`. Red-zone scenes carry the `ILLUSTRATIVE` footnote and name no real company.
- **Telemetry.** Scene counter and chapter match the scene; the egress counter climbs only in the red zone and reads `NET EGRESS 0 B` after the cut.
- **9:16.** Headline at the top inside the safe area (below ~200px), demo between y≈560 and y≈1500, captions on one line at y≈1560 clear of the demo, mark at the bottom centre, nothing right of x≈960 that matters.

## Sync

Compare the take's pauses with the voice-only film's:

```sh
ffmpeg -hide_banner -i narration-1.mp3 -af silencedetect=noise=-34dB:d=0.5 -f null - 2>&1 | grep silence_start
ffmpeg -hide_banner -i film-voice.mp4 -af silencedetect=noise=-34dB:d=0.5 -f null - 2>&1 | grep silence_start
```

The film's pauses should be the take's shifted by `NARRATION_AT`, within 0.1s. Check on the voice-only MP4: music fills the pauses in the final mix.

## Deliver

- Canonical storage: `~/Documents/anarlog-films/<epNN-slug>/` holds `script.md`, `narration.txt`, `film.html`, `film-vertical.html`, takes, `film.srt`, `music.wav` and the versioned MP4s (`ep01-v3.mp4`, `ep01-v3-vertical.mp4`). Never `~/Downloads`, which gets emptied.
- Publish `film.html` as an Artifact for interactive review (it is self-contained: inline wordmark, system fonts, no external assets).
- Send the MP4s as files. Expect roughly 6-12MB for 45s at CRF 18 (flat colour compresses well).
- Name renders by version so earlier cuts stay comparable.
- Posting: the `anarlog-film-publish` skill.
