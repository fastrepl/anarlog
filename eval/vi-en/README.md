# Vietnamese-English transcription evaluation

This directory contains a small, dependency-free scorer for comparing local
speech-to-text candidates on Vietnamese-English meeting audio. It does not
contain meeting recordings or transcripts. Keep consented audio and references
outside the repository and pass only an evaluation JSONL file to the scorer.

## Evaluation record

Create one JSON object per meeting and candidate. Keep the same reference and
critical terms for every candidate evaluated on that meeting:

```json
{
  "meeting_id": "meeting-001",
  "split": "test",
  "speaker_group": "speaker-set-a",
  "device": "iphone17-pro-max",
  "candidate": "whisper-small-q8",
  "reference": "Chưa duyệt deploy lên staging vào thứ Sáu.",
  "hypothesis": "Chưa duyệt deploy lên staging vào thứ Sáu.",
  "critical_terms": ["chưa duyệt", "staging", "thứ Sáu"],
  "audio_seconds": 60.0,
  "inference_seconds": 42.0,
  "peak_rss_mb": 940.0
}
```

`split` must be `dev` or `test`. Split by whole meeting and speaker group;
never put clips from one meeting or speaker in both splits. Use `dev` while
choosing language settings, chunk size, or model options. Reserve `test` for the
final comparison. `speaker_group` and `device` are opaque IDs; do not put
participant names, serial numbers, or other personal details in the manifest.
For each split and device, every candidate must use the same meeting set. The
scorer rejects incomplete candidate sets and reports each device separately.

`critical_terms` is an explicitly annotated list of names, product terms,
numbers, dates, negations, or other spans whose recognition affects meeting
meaning. The scorer checks exact normalized phrase presence. It does not claim
to understand whether a decision or negation was semantically preserved; review
those cases by listening to the source audio.

All durations and resource measurements are numeric and optional. Record
`inference_seconds` and `peak_rss_mb` from the same device and build being
evaluated. The scorer reports aggregate real-time factor (RTF), where lower is
faster and `1.0` is real time, plus the maximum reported peak RSS. Results are
kept separate for each `device`; do not compare resource figures across
different hardware as if they were model-only results.

For an apples-to-apples run, use the same audio file, language option, thread
count, and decoder settings for `ggml-small-q8_0.bin`,
`pho-whisper-small-f16.bin`, and `pho-whisper-small-q8_0.bin`. For example:

```sh
whisper-cli -m /path/to/model.bin -f /path/to/clip.wav -l vi -np
/usr/bin/time -l whisper-cli -m /path/to/model.bin -f /path/to/clip.wav -l vi -np
```

Repeat with `-l en` and auto-detect (omit `-l`) on the same clips. The time
command is a resource sample, not a corpus score; write per-meeting duration and
resource values into the private JSONL manifest after each run.

## Run

```sh
python3 eval/vi-en/score.py recordings.jsonl --split test
python3 -m unittest discover -s eval/vi-en -p 'test_*.py'
```

## Prepare the PhoWhisper comparison candidate

The app currently ships only Anarlog's verified Whisper Small multilingual Q8
model. To create a local PhoWhisper Small FP16 and Q8 comparison candidate,
prepare a Python environment with `numpy`, `torch`, and `transformers`, install
the Hugging Face CLI (`hf`), then run:

```sh
bash eval/vi-en/prepare-phowhisper.sh /path/with/at-least-3-GB-free
```

The script pins the PhoWhisper checkpoint and both conversion sources, verifies
the checkpoint SHA-256, builds the matching whisper.cpp converter and
quantizer, and load-checks the resulting FP16 and Q8 files against the upstream
short audio fixture. Generated weights remain outside this repository. The
upstream [PhoWhisper-small checkpoint](https://huggingface.co/vinai/PhoWhisper-small/tree/a86b604c346caf7148c37512eafe783a16420adb)
is BSD-3-Clause licensed; retain its license and research citation if a
converted artifact is ever redistributed. A successful fixture load proves
format compatibility only, not Vietnamese-English accuracy, quantization
parity, iPhone performance, or suitability for release.

Scoring uses Unicode NFKC normalization, case folding, punctuation removal,
and collapsed whitespace. WER splits on spaces; CER counts non-space Unicode
characters. Keep this normalization fixed across all candidates and report it
with any results. The corpus itself should remain private unless every
participant and source license permits sharing it.
