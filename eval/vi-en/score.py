#!/usr/bin/env python3
"""Score candidate transcripts without storing meeting data in the repo."""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
import unicodedata
from collections import defaultdict
from pathlib import Path
from typing import Any


PUNCTUATION = re.compile(r"[^\w\s]", flags=re.UNICODE)
WHITESPACE = re.compile(r"\s+")


def normalize(text: str) -> str:
    text = unicodedata.normalize("NFKC", text).casefold()
    text = PUNCTUATION.sub(" ", text)
    return WHITESPACE.sub(" ", text).strip()


def edit_distance(reference: list[str], hypothesis: list[str]) -> int:
    """Return Levenshtein distance using O(min(n, m)) memory."""
    if len(reference) < len(hypothesis):
        reference, hypothesis = hypothesis, reference
    previous = list(range(len(hypothesis) + 1))
    for row, reference_item in enumerate(reference, start=1):
        current = [row]
        for column, hypothesis_item in enumerate(hypothesis, start=1):
            current.append(
                min(
                    current[column - 1] + 1,
                    previous[column] + 1,
                    previous[column - 1] + (reference_item != hypothesis_item),
                )
            )
        previous = current
    return previous[-1]


def phrase_present(phrase: str, text: str) -> bool:
    normalized_phrase = normalize(phrase)
    if not normalized_phrase:
        return False
    return f" {normalized_phrase} " in f" {normalize(text)} "


def optional_number(record: dict[str, Any], key: str, line_number: int) -> float | None:
    value = record.get(key)
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"line {line_number}: {key} must be a number")
    value = float(value)
    if not math.isfinite(value) or value < 0:
        raise ValueError(f"line {line_number}: {key} must be finite and non-negative")
    return value


def validate_record(record: Any, line_number: int) -> dict[str, Any]:
    if not isinstance(record, dict):
        raise ValueError(f"line {line_number}: expected a JSON object")
    required_strings = (
        "meeting_id",
        "split",
        "speaker_group",
        "candidate",
        "reference",
        "hypothesis",
    )
    for key in required_strings:
        if not isinstance(record.get(key), str) or not record[key].strip():
            raise ValueError(f"line {line_number}: {key} must be a non-empty string")
    if record["split"] not in {"dev", "test"}:
        raise ValueError(f"line {line_number}: split must be 'dev' or 'test'")
    if not normalize(record["reference"]):
        raise ValueError(f"line {line_number}: reference is empty after normalization")
    critical_terms = record.get("critical_terms", [])
    if not isinstance(critical_terms, list) or not all(
        isinstance(term, str) for term in critical_terms
    ):
        raise ValueError(
            f"line {line_number}: critical_terms must be an array of strings"
        )
    record["critical_terms"] = critical_terms
    for key in ("audio_seconds", "inference_seconds", "peak_rss_mb"):
        record[key] = optional_number(record, key, line_number)
    if record["audio_seconds"] == 0:
        raise ValueError(f"line {line_number}: audio_seconds must be greater than zero")
    return record


def load_records(path: Path) -> list[dict[str, Any]]:
    records = []
    identities: set[tuple[str, str, str]] = set()
    references: dict[str, tuple[str, str, str, tuple[str, ...]]] = {}
    speaker_splits: dict[str, str] = {}
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), 1
    ):
        if not line.strip():
            continue
        try:
            record = validate_record(json.loads(line), line_number)
        except json.JSONDecodeError as error:
            raise ValueError(
                f"line {line_number}: invalid JSON: {error.msg}"
            ) from error
        identity = (record["meeting_id"], record["split"], record["candidate"])
        if identity in identities:
            raise ValueError(
                f"line {line_number}: duplicate meeting/candidate {identity[0]!r}/{identity[2]!r}"
            )
        identities.add(identity)
        reference = (
            record["split"],
            record["speaker_group"],
            normalize(record["reference"]),
            tuple(sorted(normalize(term) for term in record["critical_terms"])),
        )
        previous_reference = references.setdefault(record["meeting_id"], reference)
        if reference != previous_reference:
            raise ValueError(
                f"line {line_number}: candidates for one meeting must share split, speaker group, and reference"
            )
        previous_split = speaker_splits.setdefault(
            record["speaker_group"], record["split"]
        )
        if previous_split != record["split"]:
            raise ValueError(
                f"line {line_number}: speaker groups must not cross dev/test splits"
            )
        records.append(record)
    if not records:
        raise ValueError("input contains no evaluation records")
    return records


def score_records(
    records: list[dict[str, Any]], split: str
) -> dict[str, dict[str, float | int]]:
    selected = [record for record in records if record["split"] == split]
    if not selected:
        raise ValueError(f"input contains no records for split {split!r}")
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in selected:
        grouped[record["candidate"]].append(record)

    scores = {}
    for candidate, rows in sorted(grouped.items()):
        word_errors = word_count = char_errors = char_count = 0
        critical_hits = critical_total = 0
        audio_seconds = inference_seconds = 0.0
        has_complete_rtf = has_peak_rss = True
        peak_rss_mb = 0.0
        for record in rows:
            reference = normalize(record["reference"])
            hypothesis = normalize(record["hypothesis"])
            reference_words, hypothesis_words = reference.split(), hypothesis.split()
            reference_chars = list(reference.replace(" ", ""))
            hypothesis_chars = list(hypothesis.replace(" ", ""))
            word_errors += edit_distance(reference_words, hypothesis_words)
            word_count += len(reference_words)
            char_errors += edit_distance(reference_chars, hypothesis_chars)
            char_count += len(reference_chars)
            critical_hits += sum(
                phrase_present(term, record["hypothesis"])
                for term in record["critical_terms"]
            )
            critical_total += len(record["critical_terms"])
            if (
                record["audio_seconds"] is not None
                and record["inference_seconds"] is not None
            ):
                audio_seconds += record["audio_seconds"]
                inference_seconds += record["inference_seconds"]
            else:
                has_complete_rtf = False
            if record["peak_rss_mb"] is not None:
                peak_rss_mb = max(peak_rss_mb, record["peak_rss_mb"])
            else:
                has_peak_rss = False
        scores[candidate] = {
            "meetings": len(rows),
            "wer": word_errors / word_count,
            "cer": char_errors / char_count,
            "critical_accuracy": critical_hits / critical_total
            if critical_total
            else float("nan"),
            "rtf": inference_seconds / audio_seconds
            if has_complete_rtf and audio_seconds
            else float("nan"),
            "peak_rss_mb": peak_rss_mb if has_peak_rss else float("nan"),
        }
    return scores


def format_metric(value: float | int, suffix: str = "") -> str:
    if isinstance(value, float) and math.isnan(value):
        return "n/a"
    return f"{value:.3f}{suffix}" if isinstance(value, float) else str(value)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="private JSONL evaluation manifest")
    parser.add_argument("--split", choices=("dev", "test"), default="test")
    args = parser.parse_args()
    try:
        scores = score_records(load_records(args.input), args.split)
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    print(f"Split: {args.split}")
    print("candidate | meetings | WER | CER | critical terms | RTF | peak RSS MB")
    for candidate, score in scores.items():
        critical_accuracy = score["critical_accuracy"]
        critical_text = (
            "n/a"
            if isinstance(critical_accuracy, float) and math.isnan(critical_accuracy)
            else f"{critical_accuracy * 100:.1f}%"
        )
        print(
            f"{candidate} | {score['meetings']} | {format_metric(score['wer'])} | "
            f"{format_metric(score['cer'])} | {critical_text} | "
            f"{format_metric(score['rtf'])} | {format_metric(score['peak_rss_mb'])}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
