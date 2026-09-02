# /// script
# requires-python = ">=3.10"
# dependencies = ["datasets>=3.0", "soundfile", "numpy", "librosa"]
# ///
"""Materialise Hugging Face diarization datasets as `<stem>.wav` + `<stem>.rttm`
pairs that `diarization-eval` can score.

    uv run crates/pyannote-local/eval/fetch_datasets.py --out /tmp/diar-data \
        --dataset diarizers-community/ami:test --dataset diarizers-community/voxconverse:test

Datasets in the `diarizers-community` org share the schema
`audio, timestamps_start, timestamps_end, speakers`. Licenses: AMI and
VoxConverse are CC BY 4.0, Simsamu is MIT, CallHome (`talkbank/callhome`) is
gated and research-only. Data is for evaluation only and is never shipped.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import soundfile as sf
from datasets import Audio, load_dataset

SAMPLE_RATE = 16_000


def write_rttm(path: Path, stem: str, starts, ends, speakers) -> None:
    lines = []
    for start, end, speaker in zip(starts, ends, speakers):
        duration = float(end) - float(start)
        if duration <= 0:
            continue
        lines.append(
            f"SPEAKER {stem} 1 {float(start):.3f} {duration:.3f} <NA> <NA> {speaker} <NA> <NA>"
        )
    path.write_text("\n".join(lines) + "\n")


def materialise(spec: str, out: Path, limit: int | None, subset: str | None) -> int:
    name, _, split = spec.partition(":")
    split = split or "test"
    dataset = (
        load_dataset(name, subset, split=split)
        if subset
        else load_dataset(name, split=split)
    )
    dataset = dataset.cast_column("audio", Audio(sampling_rate=SAMPLE_RATE))
    prefix = f"{name.split('/')[-1]}_{subset + '_' if subset else ''}{split}"

    written = 0
    for index, row in enumerate(dataset):
        if limit is not None and written >= limit:
            break
        stem = f"{prefix}_{index:04d}"
        audio = np.asarray(row["audio"]["array"], dtype=np.float32)
        if audio.ndim > 1:
            audio = audio.mean(axis=1)
        sf.write(out / f"{stem}.wav", audio, SAMPLE_RATE, subtype="PCM_16")
        write_rttm(
            out / f"{stem}.rttm",
            stem,
            row["timestamps_start"],
            row["timestamps_end"],
            row["speakers"],
        )
        written += 1
    return written


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--dataset",
        action="append",
        required=True,
        help="Hub id with optional :split, e.g. diarizers-community/ami:validation",
    )
    parser.add_argument(
        "--subset", default=None, help="Config name, e.g. eng for callhome"
    )
    parser.add_argument(
        "--limit", type=int, default=None, help="Max recordings per dataset"
    )
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    for spec in args.dataset:
        count = materialise(spec, args.out, args.limit, args.subset)
        print(f"{spec}: wrote {count} recordings to {args.out}")


if __name__ == "__main__":
    main()
