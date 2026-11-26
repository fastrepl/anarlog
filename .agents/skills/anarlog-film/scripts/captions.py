"""Writes an SRT of a film's narration, timed from the audio and spelled from the script.

    uv run --with faster-whisper python3 captions.py <film.mp4> <out.srt> [narration.txt] [maxChars]

Whisper (small.en, local, CPU) gives word times. With a script (narration.txt as written; bracketed
voice directions are dropped), its words are shown instead of Whisper's, so product and technical
terms keep their spelling ("SQLite", "Parakeet", "CLOUD Act", "managed_settings.json"); unmatched
words borrow their neighbours' times. Cues are one short line each (maxChars, default 30), broken
at sentence ends, commas and pauses: what vertical-page.py draws and what YouTube, X and TikTok
take as a captions file. Read the SRT before using it.
"""

import difflib
import re
import subprocess
import sys

import numpy as np
from faster_whisper import WhisperModel

film, out = sys.argv[1], sys.argv[2]
script = open(sys.argv[3]).read() if len(sys.argv) > 3 and sys.argv[3] != "-" else None
if script:
    # Voice directions ("[short pause]", "[1s pause]") steer the TTS and are never spoken.
    script = re.sub(r"\[[^\]]*\]", " ", script)
max_chars = int(sys.argv[4]) if len(sys.argv) > 4 else 30

# Decode with ffmpeg ourselves: faster-whisper's own PyAV decoder breaks across av versions.
pcm = subprocess.run(
    [
        "ffmpeg",
        "-v",
        "error",
        "-i",
        film,
        "-ac",
        "1",
        "-ar",
        "16000",
        "-f",
        "f32le",
        "-",
    ],
    capture_output=True,
    check=True,
).stdout
model = WhisperModel("small.en", device="cpu", compute_type="int8")
segments, _ = model.transcribe(
    np.frombuffer(pcm, np.float32), word_timestamps=True, initial_prompt=script
)
heard = [
    (w.word.strip(), w.start, w.end)
    for s in segments
    for w in s.words
    if w.word.strip()
]


def norm(word):
    return re.sub(r"[^a-z0-9]", "", word.lower())


words = heard
if script:
    said = script.split()
    times = [None] * len(said)
    matcher = difflib.SequenceMatcher(
        None, [norm(w) for w in said], [norm(w) for w, _, _ in heard], autojunk=False
    )
    for block in matcher.get_matching_blocks():
        for k in range(block.size):
            times[block.a + k] = heard[block.b + k][1:]
    # Words Whisper heard differently ("to-do" vs "todo") sit between matched neighbours.
    for i, t in enumerate(times):
        if t is None:
            prev = next((times[j][1] for j in range(i - 1, -1, -1) if times[j]), 0.0)
            nxt = next(
                (times[j][0] for j in range(i + 1, len(times)) if times[j]), prev + 0.4
            )
            times[i] = (prev, max(prev, nxt))
    words = [(w, a, b) for w, (a, b) in zip(said, times)]


def chars(ws):
    return len(" ".join(w for w, _, _ in ws))


# Phrases end at a sentence, a real pause, or a comma once there is enough to read.
phrases, cur = [], []
for i, (w, a, b) in enumerate(words):
    cur.append((w, a, b))
    gap = words[i + 1][1] - b if i + 1 < len(words) else 0
    if re.search(r"[.?!]$", w) or (
        chars(cur) >= 8 and (gap > 0.6 or (w.endswith(",") and chars(cur) >= 12))
    ):
        phrases.append(cur)
        cur = []
if cur:
    phrases.append(cur)


# A phrase too long for one line splits into even parts, so no cue is a lone word.
def split(p, n):
    if n <= 1:
        return [p]
    k = min(range(1, len(p)), key=lambda k: abs(chars(p[:k]) - chars(p) / n))
    return [p[:k]] + split(p[k:], n - 1)


cues = [c for p in phrases for c in split(p, min(len(p), -(-chars(p) // max_chars)))]


def stamp(t):
    ms = int(round(t * 1000))
    return (
        f"{ms // 3600000:02}:{ms // 60000 % 60:02}:{ms // 1000 % 60:02},{ms % 1000:03}"
    )


with open(out, "w") as f:
    for n, cue in enumerate(cues):
        start = cue[0][1]
        end = cue[-1][2] + 0.35
        if n + 1 < len(cues):
            end = min(end, cues[n + 1][0][1])
        f.write(
            f"{n + 1}\n{stamp(start)} --> {stamp(end)}\n{' '.join(w for w, _, _ in cue)}\n\n"
        )
print(f"{len(cues)} cues in {out}")
