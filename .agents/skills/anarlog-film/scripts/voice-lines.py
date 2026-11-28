"""Lists the spoken stretches in a narration take and maps each to film time, so beats can be anchored.

    python3 voice-lines.py <narration.mp3> [narrationAt] [noiseDb] [minSilence]

narrationAt is when the take starts in the film (NARRATION_AT). Each stretch prints in take time and
film time. Pauses of 0.5s or more usually separate script lines; shorter ones are commas and
[short pause] tags. Use the film-time onsets as the right-hand side of TIME_MAP anchors.
"""

import re
import subprocess
import sys

take = sys.argv[1]
offset = float(sys.argv[2]) if len(sys.argv) > 2 else 0.0
noise = sys.argv[3] if len(sys.argv) > 3 else "-34dB"
min_silence = sys.argv[4] if len(sys.argv) > 4 else "0.12"

dur = float(
    subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
            take,
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
)
log = subprocess.run(
    [
        "ffmpeg",
        "-hide_banner",
        "-i",
        take,
        "-af",
        f"silencedetect=noise={noise}:d={min_silence}",
        "-f",
        "null",
        "-",
    ],
    capture_output=True,
    text=True,
).stderr
starts = [float(x) for x in re.findall(r"silence_start: ([\d.]+)", log)]
ends = [float(x) for x in re.findall(r"silence_end: ([\d.]+)", log)]

# Sound runs between silences.
edges = [0.0] + ends
stops = starts + [dur]
print(
    f"take {dur:.2f}s, film offset {offset:.2f}s, film end of voice {dur + offset:.2f}s"
)
print(f"{'#':>3}  {'take':>13}  {'film':>13}   len  pause after")
n = 0
for i, (a, b) in enumerate(zip(edges, stops)):
    if b - a < 0.08:
        continue
    n += 1
    gap = (edges[i + 1] - b) if i + 1 < len(edges) else 0.0
    mark = "  <- line break?" if gap >= 0.5 else ""
    print(
        f"{n:>3}  {a:6.2f}-{b:6.2f}  {a + offset:6.2f}-{b + offset:6.2f}  {b - a:4.2f}  {gap:4.2f}{mark}"
    )
