#!/usr/bin/env python3

import argparse
import json
import math
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass
class WavTrack:
    sample_rate: int
    samples: list[float]

    @property
    def duration_seconds(self) -> float:
        return len(self.samples) / self.sample_rate


# Shared with scripts/verify-linux-audio-qa-evidence.mjs so the producer and
# release verifier apply identical thresholds and phase rules.
POLICY = json.loads(
    (Path(__file__).with_name("linux-audio-qa-policy.json")).read_text()
)
THRESHOLDS = POLICY["thresholds"]
REQUIRED_PHASES = frozenset(POLICY["requiredPhases"])
MIN_PHASE_DURATION_SECONDS = float(POLICY["minPhaseDurationSeconds"])
PHASE_SCHEMA_VERSION = POLICY["phaseSchemaVersion"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Verify Anarlog's persisted Linux mic/system tracks from the "
            "virtual-audio QA scenario. This does not assess AEC."
        )
    )
    parser.add_argument("--mic", type=Path, required=True)
    parser.add_argument("--speaker", type=Path, required=True)
    parser.add_argument("--phases", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def read_wav(path: Path) -> WavTrack:
    data = path.read_bytes()
    if len(data) < 12 or data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError(f"{path} is not a RIFF/WAVE file")

    fmt: bytes | None = None
    audio: bytes | None = None
    cursor = 12
    while cursor + 8 <= len(data):
        chunk_id = data[cursor : cursor + 4]
        chunk_size = struct.unpack_from("<I", data, cursor + 4)[0]
        chunk_start = cursor + 8
        chunk_end = chunk_start + chunk_size
        if chunk_end > len(data):
            raise ValueError(f"{path} has a truncated {chunk_id!r} chunk")
        if chunk_id == b"fmt ":
            fmt = data[chunk_start:chunk_end]
        elif chunk_id == b"data":
            audio = data[chunk_start:chunk_end]
        cursor = chunk_end + (chunk_size % 2)

    if fmt is None or len(fmt) < 16 or audio is None:
        raise ValueError(f"{path} is missing required WAV chunks")

    (
        audio_format,
        channels,
        sample_rate,
        _byte_rate,
        block_align,
        bits_per_sample,
    ) = struct.unpack_from("<HHIIHH", fmt)
    if audio_format == 0xFFFE and len(fmt) >= 40:
        audio_format = struct.unpack_from("<H", fmt, 24)[0]

    if channels != 1:
        raise ValueError(f"{path} must be mono, got {channels} channels")
    if block_align != bits_per_sample // 8:
        raise ValueError(f"{path} has an unsupported WAV block alignment")

    samples = decode_samples(audio, audio_format, bits_per_sample)
    if not samples:
        raise ValueError(f"{path} has no audio samples")
    if not all(math.isfinite(sample) for sample in samples):
        raise ValueError(f"{path} contains non-finite samples")

    return WavTrack(sample_rate=sample_rate, samples=samples)


def decode_samples(data: bytes, audio_format: int, bits_per_sample: int) -> list[float]:
    if audio_format == 3 and bits_per_sample == 32:
        if len(data) % 4:
            raise ValueError("float WAV data is not sample-aligned")
        return list(struct.unpack(f"<{len(data) // 4}f", data))

    if audio_format != 1:
        raise ValueError(
            f"unsupported WAV format {audio_format} with {bits_per_sample} bits"
        )

    if bits_per_sample == 16:
        if len(data) % 2:
            raise ValueError("PCM16 WAV data is not sample-aligned")
        return [
            sample / 32768.0 for sample in struct.unpack(f"<{len(data) // 2}h", data)
        ]
    if bits_per_sample == 24:
        if len(data) % 3:
            raise ValueError("PCM24 WAV data is not sample-aligned")
        samples = []
        for offset in range(0, len(data), 3):
            value = int.from_bytes(data[offset : offset + 3], "little", signed=False)
            if value & 0x800000:
                value -= 1 << 24
            samples.append(value / 8388608.0)
        return samples
    if bits_per_sample == 32:
        if len(data) % 4:
            raise ValueError("PCM32 WAV data is not sample-aligned")
        return [
            sample / 2147483648.0
            for sample in struct.unpack(f"<{len(data) // 4}i", data)
        ]

    raise ValueError(f"unsupported PCM bit depth {bits_per_sample}")


def load_phases(path: Path) -> tuple[dict[str, dict[str, float]], float]:
    payload = json.loads(path.read_text())
    if payload.get("schema_version") != PHASE_SCHEMA_VERSION:
        raise ValueError("unsupported phase schema")
    if payload.get("no_aec") is not True:
        raise ValueError("phase evidence must declare no_aec=true")

    phases: dict[str, dict[str, float]] = {}
    for phase in payload.get("phases", []):
        name = phase.get("name")
        if name not in REQUIRED_PHASES:
            continue
        start = float(phase["start_seconds"])
        end = float(phase["end_seconds"])
        if end - start < MIN_PHASE_DURATION_SECONDS:
            raise ValueError(f"{name} phase is too short for deterministic analysis")
        phases[name] = {"start": start, "end": end}

    missing = REQUIRED_PHASES - phases.keys()
    if missing:
        raise ValueError(f"missing phase evidence: {', '.join(sorted(missing))}")

    recording_stop = float(payload.get("recording_stop_seconds", 0))
    if recording_stop <= max(phase["end"] for phase in phases.values()):
        raise ValueError("recording stop must follow every signal phase")
    return phases, recording_stop


def window(
    track: WavTrack,
    phase: dict[str, float],
    offset_seconds: float,
    trim_seconds: float = 1.25,
) -> list[float]:
    start = phase["start"] + offset_seconds + trim_seconds
    end = phase["end"] + offset_seconds - trim_seconds
    if end <= start:
        raise ValueError("phase is too short after trimming")
    start_index = round(start * track.sample_rate)
    end_index = round(end * track.sample_rate)
    if start_index < 0 or end_index > len(track.samples):
        raise ValueError("phase window falls outside the persisted track")
    return track.samples[start_index:end_index]


def rms(samples: list[float]) -> float:
    return math.sqrt(math.fsum(sample * sample for sample in samples) / len(samples))


def goertzel_amplitude(
    samples: list[float], sample_rate: int, frequency: float
) -> float:
    angular_frequency = 2.0 * math.pi * frequency / sample_rate
    coefficient = 2.0 * math.cos(angular_frequency)
    previous = 0.0
    previous_previous = 0.0
    for sample in samples:
        current = sample + coefficient * previous - previous_previous
        previous_previous = previous
        previous = current
    power = (
        previous_previous * previous_previous
        + previous * previous
        - coefficient * previous * previous_previous
    )
    return 2.0 * math.sqrt(max(power, 0.0)) / len(samples)


def ratio_db(active: float, quiet: float) -> float:
    floor = 1e-12
    return 20.0 * math.log10(max(active, floor) / max(quiet, floor))


def find_alignment(speaker: WavTrack, phases: dict[str, dict[str, float]]) -> float:
    candidates: list[tuple[float, float]] = []
    for step in range(-20, 81):
        offset = step / 10.0
        try:
            mic_only = window(speaker, phases["mic_only"], offset)
            system_only = window(speaker, phases["system_only"], offset)
            both = window(speaker, phases["both"], offset)
        except ValueError:
            continue

        quiet_tone = goertzel_amplitude(mic_only, speaker.sample_rate, 997.0)
        active_tone = min(
            goertzel_amplitude(system_only, speaker.sample_rate, 997.0),
            goertzel_amplitude(both, speaker.sample_rate, 997.0),
        )
        score = active_tone / max(quiet_tone, 1e-9)
        candidates.append((score, offset))

    if not candidates:
        raise ValueError("could not align phase evidence with the persisted tracks")
    best_score = max(score for score, _ in candidates)
    plateau = [offset for score, offset in candidates if score >= best_score * 0.98]
    return (min(plateau) + max(plateau)) / 2.0


def clipped_fraction(track: WavTrack) -> float:
    return sum(1 for sample in track.samples if abs(sample) >= 0.999) / len(
        track.samples
    )


def analyze(
    mic: WavTrack,
    speaker: WavTrack,
    phases: dict[str, dict[str, float]],
    recording_stop_seconds: float,
) -> dict[str, Any]:
    if mic.sample_rate != 16_000 or speaker.sample_rate != 16_000:
        raise ValueError(
            "debug tracks must both be 16 kHz "
            f"(mic={mic.sample_rate}, speaker={speaker.sample_rate})"
        )

    alignment = find_alignment(speaker, phases)
    mic_windows = {
        name: window(mic, phase, alignment) for name, phase in phases.items()
    }
    speaker_windows = {
        name: window(speaker, phase, alignment) for name, phase in phases.items()
    }

    metrics = {
        "mic_duration_seconds": mic.duration_seconds,
        "speaker_duration_seconds": speaker.duration_seconds,
        "duration_delta_seconds": abs(mic.duration_seconds - speaker.duration_seconds),
        "alignment_offset_seconds": alignment,
        "recording_stop_seconds": recording_stop_seconds,
        "aligned_recording_stop_seconds": recording_stop_seconds + alignment,
        "mic_recording_tail_seconds": (
            mic.duration_seconds - recording_stop_seconds - alignment
        ),
        "speaker_recording_tail_seconds": (
            speaker.duration_seconds - recording_stop_seconds - alignment
        ),
        "mic_clipped_fraction": clipped_fraction(mic),
        "speaker_clipped_fraction": clipped_fraction(speaker),
        "mic_only_mic_rms": rms(mic_windows["mic_only"]),
        "system_only_mic_rms": rms(mic_windows["system_only"]),
        "both_mic_rms": rms(mic_windows["both"]),
        "mic_only_speaker_rms": rms(speaker_windows["mic_only"]),
        "system_only_speaker_rms": rms(speaker_windows["system_only"]),
        "both_speaker_rms": rms(speaker_windows["both"]),
        "mic_only_523hz_amplitude": goertzel_amplitude(
            mic_windows["mic_only"], mic.sample_rate, 523.0
        ),
        "system_only_523hz_amplitude": goertzel_amplitude(
            mic_windows["system_only"], mic.sample_rate, 523.0
        ),
        "both_523hz_amplitude": goertzel_amplitude(
            mic_windows["both"], mic.sample_rate, 523.0
        ),
        "mic_only_997hz_amplitude": goertzel_amplitude(
            speaker_windows["mic_only"], speaker.sample_rate, 997.0
        ),
        "system_only_997hz_amplitude": goertzel_amplitude(
            speaker_windows["system_only"], speaker.sample_rate, 997.0
        ),
        "both_997hz_amplitude": goertzel_amplitude(
            speaker_windows["both"], speaker.sample_rate, 997.0
        ),
        "both_mic_997hz_amplitude": goertzel_amplitude(
            mic_windows["both"], mic.sample_rate, 997.0
        ),
        "both_speaker_523hz_amplitude": goertzel_amplitude(
            speaker_windows["both"], speaker.sample_rate, 523.0
        ),
    }
    metrics.update(
        {
            "mic_only_isolation_db": ratio_db(
                metrics["mic_only_mic_rms"], metrics["system_only_mic_rms"]
            ),
            "both_mic_isolation_db": ratio_db(
                metrics["both_mic_rms"], metrics["system_only_mic_rms"]
            ),
            "system_only_speaker_isolation_db": ratio_db(
                metrics["system_only_speaker_rms"],
                metrics["mic_only_speaker_rms"],
            ),
            "both_speaker_isolation_db": ratio_db(
                metrics["both_speaker_rms"], metrics["mic_only_speaker_rms"]
            ),
            "system_tone_isolation_db": ratio_db(
                min(
                    metrics["system_only_997hz_amplitude"],
                    metrics["both_997hz_amplitude"],
                ),
                metrics["mic_only_997hz_amplitude"],
            ),
            "mic_pilot_isolation_db": ratio_db(
                min(
                    metrics["mic_only_523hz_amplitude"],
                    metrics["both_523hz_amplitude"],
                ),
                metrics["system_only_523hz_amplitude"],
            ),
            "both_system_to_mic_isolation_db": ratio_db(
                metrics["both_997hz_amplitude"],
                metrics["both_mic_997hz_amplitude"],
            ),
            "both_mic_to_speaker_isolation_db": ratio_db(
                metrics["both_523hz_amplitude"],
                metrics["both_speaker_523hz_amplitude"],
            ),
        }
    )

    failures = []
    require_at_most(
        failures,
        "duration delta",
        metrics["duration_delta_seconds"],
        THRESHOLDS["duration_delta_seconds_max"],
    )
    require_at_least(
        failures,
        "mic recording tail",
        metrics["mic_recording_tail_seconds"],
        -THRESHOLDS["recording_tail_tolerance_seconds"],
    )
    require_at_least(
        failures,
        "speaker recording tail",
        metrics["speaker_recording_tail_seconds"],
        -THRESHOLDS["recording_tail_tolerance_seconds"],
    )
    require_at_most(
        failures,
        "mic recording tail",
        metrics["mic_recording_tail_seconds"],
        THRESHOLDS["recording_tail_tolerance_seconds"],
    )
    require_at_most(
        failures,
        "speaker recording tail",
        metrics["speaker_recording_tail_seconds"],
        THRESHOLDS["recording_tail_tolerance_seconds"],
    )
    require_at_most(
        failures,
        "mic clipped fraction",
        metrics["mic_clipped_fraction"],
        THRESHOLDS["clipped_fraction_max"],
    )
    require_at_most(
        failures,
        "speaker clipped fraction",
        metrics["speaker_clipped_fraction"],
        THRESHOLDS["clipped_fraction_max"],
    )
    for key in ("mic_only_mic_rms", "both_mic_rms"):
        require_at_least(failures, key, metrics[key], THRESHOLDS["mic_active_rms_min"])
    for key in ("system_only_speaker_rms", "both_speaker_rms"):
        require_at_least(
            failures, key, metrics[key], THRESHOLDS["speaker_active_rms_min"]
        )
    for key in ("system_only_997hz_amplitude", "both_997hz_amplitude"):
        require_at_least(
            failures,
            key,
            metrics[key],
            THRESHOLDS["system_tone_amplitude_min"],
        )
    for key in ("mic_only_523hz_amplitude", "both_523hz_amplitude"):
        require_at_least(
            failures,
            key,
            metrics[key],
            THRESHOLDS["mic_pilot_amplitude_min"],
        )
    for key in ("mic_only_isolation_db", "both_mic_isolation_db"):
        require_at_least(
            failures, key, metrics[key], THRESHOLDS["mic_isolation_db_min"]
        )
    for key in (
        "system_only_speaker_isolation_db",
        "both_speaker_isolation_db",
    ):
        require_at_least(
            failures, key, metrics[key], THRESHOLDS["speaker_isolation_db_min"]
        )
    require_at_least(
        failures,
        "system_tone_isolation_db",
        metrics["system_tone_isolation_db"],
        THRESHOLDS["system_tone_isolation_db_min"],
    )
    require_at_least(
        failures,
        "mic_pilot_isolation_db",
        metrics["mic_pilot_isolation_db"],
        THRESHOLDS["mic_pilot_isolation_db_min"],
    )
    for key in (
        "both_system_to_mic_isolation_db",
        "both_mic_to_speaker_isolation_db",
    ):
        require_at_least(
            failures,
            key,
            metrics[key],
            THRESHOLDS["cross_channel_isolation_db_min"],
        )

    return {
        "status": "pass" if not failures else "fail",
        "scope": (
            "Virtual microphone/system routing, capture, separation, and "
            "persistence with NO_AEC=1. This result makes no AEC claim."
        ),
        "thresholds": THRESHOLDS,
        "metrics": metrics,
        "failures": failures,
    }


def require_at_least(
    failures: list[str], name: str, actual: float, minimum: float
) -> None:
    if actual < minimum:
        failures.append(f"{name} was {actual:.8g}, expected at least {minimum:.8g}")


def require_at_most(
    failures: list[str], name: str, actual: float, maximum: float
) -> None:
    if actual > maximum:
        failures.append(f"{name} was {actual:.8g}, expected at most {maximum:.8g}")


def write_result(path: Path, result: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")


def main() -> int:
    args = parse_args()
    try:
        phases, recording_stop_seconds = load_phases(args.phases)
        result = analyze(
            read_wav(args.mic),
            read_wav(args.speaker),
            phases,
            recording_stop_seconds,
        )
    except Exception as error:
        result = {
            "status": "fail",
            "scope": (
                "Virtual microphone/system routing, capture, separation, and "
                "persistence with NO_AEC=1. This result makes no AEC claim."
            ),
            "failures": [str(error)],
        }

    write_result(args.output, result)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["status"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
