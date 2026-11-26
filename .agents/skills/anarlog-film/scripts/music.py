"""Music for Anarlog films: one instrumental take, cut once into three parts every film reuses, then
laid out and levelled to fit each film. See references/music.md for the theme and the Suno prompt.

    python3 music.py cut <take.wav> <musicDir> <bpm> <downbeat> <opening> <bed> <ending>
    python3 music.py bed <musicDir> <narrationAt> <endAt> <music.wav>

cut: writes opening.wav, bed.wav and ending.wav. downbeat is the take time of bar 0; opening, bed
and ending are bar ranges such as -2, 8-16 and 40-. An open start (-2) begins at the take's first
sample, keeping any pickup before bar 0; an open end (40-) runs to the end of the take. Bars are 4
beats. The bed must loop: its last bar has to lead back into its first.

bed: lays the parts out for one film. The opening starts at 0. The ending starts at endAt, just after
the narration's last word, and the film runs until the ending has rung out: bed prints that
`film_end`, which must be the film's last TIME_MAP time (hold the end card until then). The bed loops
in between, counted back from endAt so whole bars lead into the ending, and dissolves in under the
opening's last second. The music sits at LEVEL_LUFS with no voice over it and a steady UNDER_DB
lower from narrationAt to endAt: no ducking that swells in the pauses. The take is never touched.
"""

import json
import math
import os
import re
import subprocess
import sys

XFADE = 1.0  # opening -> bed dissolve
EDGE = 0.02  # declick at bar cuts
TAIL = 1.0  # fade at the film's last second
LEVEL_LUFS = (
    -27.0
)  # the opening and ending, with no voice over them: a little under the voice
UNDER_DB = (
    -15.0
)  # under the narration: the dark pulse sits ~20 LU below a v4 take (around -22 LUFS)
RAMP_DOWN = 0.8  # into the narration, finishing as the voice starts
RAMP_UP = 0.3  # back up into the ending's first downbeat


def run(args):
    return subprocess.run(args, check=True, capture_output=True, text=True)


def length(path):
    return float(
        run(
            [
                "ffprobe",
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
                path,
            ]
        ).stdout
    )


def cut(take, out, bpm, downbeat, *ranges):
    bar = 240.0 / float(bpm)
    os.makedirs(out, exist_ok=True)
    for name, spec in zip(["opening", "bed", "ending"], ranges):
        a, _, b = spec.partition("-")
        start = float(downbeat) + int(a) * bar if a else 0.0
        span = ["-t", f"{float(downbeat) + int(b) * bar - start:.4f}"] if b else []
        run(
            [
                "ffmpeg",
                "-y",
                "-loglevel",
                "error",
                "-ss",
                f"{start:.4f}",
                *span,
                "-i",
                take,
                "-ar",
                "48000",
                "-ac",
                "2",
                f"{out}/{name}.wav",
            ]
        )
        print(
            f"{name}: bars {spec}, {start:.3f}s, {length(f'{out}/{name}.wav'):.3f}s long"
        )


def rings_out(path):
    """Seconds until the last sound above -50dB from the peak."""
    pcm = subprocess.run(
        [
            "ffmpeg",
            "-loglevel",
            "error",
            "-i",
            path,
            "-ac",
            "1",
            "-ar",
            "8000",
            "-f",
            "s16le",
            "-",
        ],
        check=True,
        capture_output=True,
    ).stdout
    x = [abs(v) for v in memoryview(pcm).cast("h")]
    floor = max(x) * 10 ** (-50 / 20)
    return max(i for i, v in enumerate(x) if v > floor) / 8000


def bed(src, narration_at, end_at, out):
    narration_at, end_at = float(narration_at), float(end_at)
    opening, loop, ending = (f"{src}/{n}.wav" for n in ["opening", "bed", "ending"])
    o, b = length(opening), length(loop)
    duration = math.ceil((end_at + rings_out(ending) + 0.1) * 10) / 10
    # Each piece: [input, trim start, trim length, film start, fade in, fade out].
    pieces = [[0, 0.0, min(o, end_at), 0.0, EDGE, XFADE]]
    s = end_at - b
    while s + b > o - XFADE and end_at > o:
        head = max(0.0, o - XFADE - s)
        # Dissolve in under the opening; a later bed segment only needs a declick.
        pieces.append(
            [1, head, b - head, s + head, XFADE if s + head < o else EDGE, EDGE]
        )
        s -= b
    pieces.append([2, 0.0, duration - end_at, end_at, EDGE, TAIL])

    # One input per piece: reusing one input across several delayed branches stalls ffmpeg.
    files = {0: opening, 1: loop, 2: ending}
    inputs, graph, labels = [], [], []
    for i, (src_i, ss, n, at, fin, fout) in enumerate(pieces):
        inputs += ["-i", files[src_i]]
        graph.append(
            f"[{i}:a]atrim={ss:.4f}:{ss + n:.4f},asetpts=PTS-STARTPTS,"
            f"afade=in:d={fin},afade=out:st={max(0.0, n - fout):.4f}:d={fout},"
            f"adelay={round(at * 1000)}:all=1[p{i}]"
        )
        labels.append(f"[p{i}]")
    graph.append(
        f"{''.join(labels)}amix=inputs={len(labels)}:normalize=0,apad,atrim=0:{duration:.4f}[m]"
    )
    tmp = out + ".raw.wav"
    run(
        [
            "ffmpeg",
            "-y",
            "-loglevel",
            "error",
            *inputs,
            "-filter_complex",
            ";".join(graph),
            "-map",
            "[m]",
            "-ar",
            "48000",
            "-ac",
            "2",
            tmp,
        ]
    )

    # A plain gain to LEVEL_LUFS, so any take sits at the same level, then a steady UNDER_DB
    # from narrationAt to endAt: no ducking that swells in the pauses.
    log = run(
        ["ffmpeg", "-hide_banner", "-i", tmp, "-af", "ebur128", "-f", "null", "-"]
    ).stderr
    gain = LEVEL_LUFS - float(re.findall(r"I:\s+(-?[\d.]+) LUFS", log)[-1])
    under = f"clip(min((t-{narration_at - RAMP_DOWN:.3f})/{RAMP_DOWN},({end_at:.3f}-t)/{RAMP_UP}),0,1)"
    run(
        [
            "ffmpeg",
            "-y",
            "-loglevel",
            "error",
            "-i",
            tmp,
            "-af",
            f"volume='pow(10,({gain:.2f}+{UNDER_DB}*{under})/20)':eval=frame",
            out,
        ]
    )
    run(["rm", tmp])
    print(
        json.dumps(
            {
                "opening": round(o, 3),
                "bed": round(b, 3),
                "loops": len(pieces) - 2,
                "ending_at": end_at,
                "film_end": duration,
                "gain_db": round(gain, 2),
            }
        )
    )


if __name__ == "__main__":
    {"cut": cut, "bed": bed}[sys.argv[1]](*sys.argv[2:])
