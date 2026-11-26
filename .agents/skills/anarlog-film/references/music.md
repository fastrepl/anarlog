# Music

Every Anarlog film uses the same instrumental theme, cut once into three parts that every film reuses:

- **Opening** (about 2 bars) under the wordmark wipe and the opening line.
- **Bed**, looped quietly under the narration.
- **Ending**, from just after the last word, under the end card, ringing out. The film holds the end card until it has.

One take means one key, tempo and sound across the series, and seamless joins. The narration is never touched: only the music is cut, levelled and held under it.

The sound is the opposite of a feel-good product jingle: **minimal architectural electronics with a muted analog dark pulse**. Cold and precise under the red-zone scenes, warming slightly (not resolving into a pop chorus) by the end card. Never reuse Char's funk theme.

## Getting the parts

Once the theme exists, its take and parts live in the public static bucket under `anarlog/films/music/` (R2 `fastrepl-static-assets`, served at `https://static.fastrepl.com/anarlog/films/music/...`). Fetch them into the film folder:

```sh
B=https://static.fastrepl.com/anarlog/films/music; curl -fsS --create-dirs -o music/opening.wav $B/<opening file> -o music/bed.wav $B/<bed file> -o music/ending.wav $B/<ending file>
```

Record the take and each part here with its URL, size, duration and SHA-1 once uploaded:

| File    | URL       | Size | SHA-1 |
| ------- | --------- | ---- | ----- |
| Take    | (pending) |      |       |
| Opening | (pending) |      |       |
| Bed     | (pending) |      |       |
| Ending  | (pending) |      |       |

Upload with `apps/web/scripts/r2-assets.mjs` under John's own `wrangler login`. Published objects are never replaced: a new cut gets new filenames (content hash in the name), so earlier films keep their parts.

## Making the theme (Suno)

Generate only with John's OK, on a paid plan (Suno grants commercial use only for songs made while subscribed), within the credits already on the account. Never buy credits or change the plan.

- **Custom** (Advanced) mode, latest model.
- **Style:**

  > instrumental, minimal architectural electronic, muted analog synth pulse, 96 BPM, A minor, dry tight kick, soft closed hats, deep sub bass, filtered arpeggio, sparse glassy plucks, cold precise mix, restrained, steady tempo, no lead melody, documentary tension

- **Exclude styles:**

  > vocals, voice, choir, humming, funk, brass, saxophone, guitar solo, risers, EDM drop, trap hats, distortion, cinematic hits, piano ballad

- **Lyrics box:** section tags only, no words:

  ```
  [Intro]
  [Instrumental: analog pulse alone, filter slowly opening, clean downbeat]
  [Verse]
  [Instrumental: kick and sub enter, steady pulse, no melody]
  [Verse]
  [Instrumental: same groove, add sparse glassy plucks]
  [Outro]
  [Instrumental: pulse thins out, one warm sustained chord, ring out]
  [End]
  ```

Pick the take whose tempo holds through the bed, whose bed has no lead line competing with the voice, and which ends on a clean sustained chord. If the ending is weak, use **Extend** from the end of the bed with `[Outro] [End]`. Download the WAV.

## Cutting the parts

Find the take's tempo and its first downbeat (bar 0) from a low-frequency onset plot of the kick; check the final chord sits on the same grid. Then choose bar ranges:

- **Opening:** about 5s. `-2` starts at the take's first sample, keeping any pickup.
- **Bed:** one 8-bar phrase that loops (its last bar leads back into its first).
- **Ending:** one bar before the final chord, to the end.

```sh
python3 $SK/scripts/music.py cut take.wav music <bpm> <downbeat> -2 8-16 <lastBar-1>-
```

This writes `music/opening.wav`, `music/bed.wav` and `music/ending.wav`. Cut again only if the take changes; then upload and update the table above.

## Laying it under a film

```sh
python3 $SK/scripts/music.py bed music <NARRATION_AT> <endAt> music.wav
MUSIC=music.wav bash $SK/scripts/mux.sh frames film.mp4 narration-1.mp3 <NARRATION_AT>
```

- `endAt` is about 0.15s after the narration's last word, in film time (the last stretch `voice-lines.py` prints).
- `music.py bed` prints `film_end`, when the ending has rung out. Make it the film's last `TIME_MAP` time by holding the end card (`[DURATION, ...], [DURATION, film_end]`), then render the frames.
- Levels: opening and ending at -27 LUFS; from `NARRATION_AT` to `endAt` the music sits a steady 15dB lower (about 20 LU under a v4 take), easing down over 0.8s as the voice starts and back up in the 0.3s before the ending. No ducking, so it never swells in the pauses.
- Too loud or too quiet: change `LEVEL_LUFS` or `UNDER_DB` in `music.py`. Never change the take.

## Before the theme exists

Mux the voice-only cut (no `MUSIC`) and say so in the delivery. Do not borrow Char's music or stock tracks.

## Checking

The music fills the take's pauses, so check sync on the voice-only mux (see [verify.md](verify.md)). Then listen to the music mix at three places: the first line (music settles low as the voice starts), a long pause (stays at the same level), and the last word (the ending comes up and rings out before the film ends).
