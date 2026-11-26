"""Builds the native 9:16 page of a finished anarlog-film page: the same scenes, laid out at 1080x1920.

    python3 vertical-page.py <film.html> <film-vertical.html> --srt film.srt [--replace vertical.json]

Then render and mux it like the 16:9 cut:

    node render.mjs film-vertical.html frames frames-vertical        # size comes from the page
    MUSIC=music.wav bash mux.sh frames-vertical film-vertical.mp4 narration.mp3 <NARRATION_AT>

The template lays itself out from one constant, FORMAT. Flipping it to "9:16" switches the stage to
1080x1920 and applies the template's vertical rules: the copy column runs full width from y=230
(headline at 76px, footnote hidden), the demo box sits at x=100, y=640..1520, the top rail drops its
dock slot, and the wordmark docks at the bottom centre (y~1765) instead of the top rail. The page
then draws the captions: one square, solid line at y=1560, above the docked mark.

A film whose own demo needs other placement in 9:16 adds `.stage[data-format="9:16"] ...` rules to
its own CSS, or passes --replace: a JSON list of [old, new] pairs, each of which must match once.
Everything stays inside the area Reels, Shorts and TikTok leave clear: below ~200px at the top,
left of ~120px at the right, above ~400px at the bottom (only the mark and the rail sit there).
"""

import argparse
import json
import pathlib
import re

ap = argparse.ArgumentParser()
ap.add_argument("film")
ap.add_argument("out")
ap.add_argument("--srt", required=True, help="captions from captions.py")
ap.add_argument(
    "--replace", help="JSON list of [old, new] pairs for this film's 9:16 layout"
)
ap.add_argument(
    "--max-chars", type=int, default=36, help="warn on cues longer than this"
)
args = ap.parse_args()
s = pathlib.Path(args.film).read_text()


def R(old, new):
    global s
    n = s.count(old)
    if n != 1:
        raise SystemExit(f"expected exactly one match, found {n}: {old[:80]!r}")
    s = s.replace(old, new)


R('const FORMAT = "16:9";', 'const FORMAT = "9:16";')

for old, new in (
    json.loads(pathlib.Path(args.replace).read_text()) if args.replace else []
):
    R(old, new)

# Captions run on the film clock T (they are timed from the take), not scene time.
cues = re.findall(
    r"(\d[\d:,.]*) --> (\d[\d:,.]*)\s*\n(.+?)(?:\n\s*\n|\s*$)",
    pathlib.Path(args.srt).read_text().replace("\r\n", "\n"),
    re.S,
)
if not cues:
    raise SystemExit(f"no cues in {args.srt}")


def stamp(v):
    return sum(
        float(x) * m for x, m in zip(v.replace(",", ".").split(":"), (3600, 60, 1))
    )


rows = []
for a, b, text in cues:
    line = " ".join(text.split())
    if len(line) > args.max_chars:
        print(
            f"warning: cue longer than {args.max_chars} characters may not fit one line: {line!r}"
        )
    rows.append(f"[{stamp(a):.3f}, {stamp(b):.3f}, {json.dumps(line)}]")
R("const CUES = [];", "const CUES = [\n      " + ",\n      ".join(rows) + ",\n    ];")

pathlib.Path(args.out).write_text(s)
print(f"{args.out}: 1080x1920, {len(cues)} captions")
