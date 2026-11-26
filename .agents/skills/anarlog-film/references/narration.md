# Narration

The voiceover is one take, generated from one script, played as generated. The film is timed to it, never the other way round.

## Script

- **John's direct engineering voice.** Short declaratives, concrete nouns, no hype words ("revolutionary", "seamless", "game-changing"), no questions after the opening. Say what happens to the data: where it goes, who can read it, how long it stays.
- **One line per beat**, 5-7 beats, about 38-45s read at a calm pace (roughly 2.6 words per second plus pauses). Read it aloud: if it rushes, cut words; if a beat needs more time, add a clause that describes what is on screen.
- **Straight ASCII quotes and apostrophes only** (`'` and `"`), no curly quotes. **No em dashes or en dashes**: use a period or a comma. The same rule applies to on-screen copy, captions and posts.
- **Spell for the ear.** `SQLite` reads fine; write `M-I-T` only if a take mispronounces `MIT`; write `E-U` if `EU` is read as a word. Keep the written script (`narration.txt`) in the spelling the captions should show, and note any ear-spellings beside it in `script.md`.
- **Pacing tags** (Eleven v3/v4): `[short pause]` inside a line, `[long pause]` between beats, `[1s pause]` before the end card. No mood tags (`[softly]`, `[excited]`).
- **The end card line** is the promise, said slowly: `Anarlog. [long pause] Your meetings stay on your machine.`
- Every factual line needs a source in `script.md`. Keep a "Must not say" list there too.

Keep `script.md` (beats, headlines, on-screen copy, claim sources, must-not-say list) and `narration.txt` (exactly what goes into the TTS box) in the film folder, so a later take regenerates the same text with only the intended edits.

## The voice

John's Professional Voice Clone on his ElevenLabs account:

| Setting   | Value                                        |
| --------- | -------------------------------------------- |
| Voice     | **John - Passionate Founder**                |
| Voice ID  | `PXo7njgt8slUVdtKa3La`                       |
| Model     | **Eleven v4**                                |
| Stability | **Robust** (100%)                            |
| Output    | MP3 44.1 kHz, downloaded as `narration-N.mp3` |

Voices get renamed; confirm the voice ID before generating. A downloaded filename keeps the clone's original name, which shows whether it is the same voice.

**Credits.** Each take costs about one credit per character. Never generate without John's OK for that take. Say what changed and why before regenerating, and generate one take at a time.

## Generating the take

Use the first route that is available:

1. **An ElevenLabs connector** in the host, signed in to John's account: find the voice by ID, call speech generation with `model_id: "eleven_v4"`, one generation, the whole `narration.txt` as the prompt. Price it with an estimate-only call first. Connectors may not expose Stability; if the read is livelier than Robust, use the browser route.
2. **John's signed-in ElevenLabs tab** (for example through the Aside browser): Text to Speech, model Eleven v4, voice `John - Passionate Founder`, Stability Robust.
3. **An API key from John**, never committed: `POST /v1/text-to-speech/PXo7njgt8slUVdtKa3La` with `model_id` set to the v4 model.

### The v4 multiline editor gotcha

The Eleven v4 text box is a rich editor. A single multiline `fill()` (or a paste of the whole script through automation) can drop lines silently: the box looks right at a glance, but the character counter shows fewer characters than the script, and the take ends early.

- Clear the editor, then insert the script **line by line**, pressing **Enter** between paragraphs (beats) and **Shift+Enter** for a line break inside a paragraph.
- Before pressing Generate, compare the **displayed character counter** with the script's length (`wc -m narration.txt`, minus trailing newline; tags count as characters). They must match. If they do not, clear and insert again.
- When editing a later take, select only the changed words between tags and type over them, so the rest of the script and its tags stay exactly as John left them.

Downloads land in `~/Downloads`; copy the take into the film folder as `narration-N.mp3` straight away.

## Mapping the take

```sh
python3 $SK/scripts/voice-lines.py narration-1.mp3 <NARRATION_AT>
```

It prints each spoken stretch in take time and film time and marks pauses of 0.5s or more as likely line breaks. Match the stretches to the script's lines and phrases and note each phrase's film-time onset: those are the times beats must land on. A one-word line such as "Anarlog." is a short stretch (about 0.5s) followed by a long pause; do not merge it into the line before.

## Building TIME_MAP

`TIME_MAP` is a list of `[scene time, film time]` pairs. Between pairs, scenes play at whatever speed makes the next pair land.

- `NARRATION_AT` is where the take starts in the film: just as the opening line starts typing (about 2.2s, after the wordmark docks).
- One anchor per spoken phrase: the scene time of the beat it describes (headline in, bot tile flips, row typed, route lit, key locked) paired with `NARRATION_AT + onset`.
- Hold a frame by repeating a scene time with a later film time: `[18.2, 19.1], [18.2, 20.0]`. Put holds where everything is on screen and still.
- Cut ahead by repeating a film time with a later scene time.
- Keep speeds between 0.6x and 1.6x. Anchor the polarity cut (`CUT.at`) to the gap before the first Anarlog line, so the wipe lands in silence and the line starts on newsprint.
- End with `[DURATION, end], [DURATION, film_end]`: `end` leaves about 1.2s after the last word, and `film_end` comes from `music.py bed`.
- When a beat cannot land, change the scene's internal timing rather than squeezing the map.

After mapping, render shots at each anchor's film time and check that the frame shows what the voice is saying.
