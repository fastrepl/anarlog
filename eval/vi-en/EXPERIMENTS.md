# Local model conversion smoke check

Date: 2026-11-02

This is a format/runtime smoke check, not the planned Vietnamese-English
meeting evaluation. It used the upstream `whisper.cpp` `samples/jfk.wav`
fixture (about 11 seconds) on an Apple ARM laptop with 16 GiB RAM. All runs used
`whisper-cli`, `-l en`, default thread settings, and one invocation per model.
Runtime and peak RSS are single-run observations.

| Model | Output | Wall time | Peak RSS |
| --- | --- | ---: | ---: |
| Anarlog Whisper Small multilingual Q8_0 | Correct JFK reference sentence | 2.36 s | 568 MB |
| PhoWhisper Small FP16 | Repeated/omitted English words; segment extended to 30 s | 5.92 s | 771 MB |
| PhoWhisper Small Q8_0 | Same repeated/omitted English output; segment extended to 30 s | 4.49 s | 571 MB |

The PhoWhisper conversion and Q8 quantization load successfully. Its result on
this English-only fixture is poor, so this run does not support choosing it as
the multilingual default. It does not measure Vietnamese accuracy, mixed
Vietnamese-English quality, quantization parity on the target domain, or iPhone
performance. Keep both PhoWhisper artifacts as local evaluation candidates;
do not ship them or treat this smoke check as the held-out corpus result.

The weights were generated from the pinned checkpoint and converter revisions
in `prepare-phowhisper.sh`. They are not stored in this repository. No private
meeting audio or transcript was used.

# iOS simulator local-transcription smoke check

Date: 2026-11-02

An iPhone 17 Pro Max simulator running iOS 26.5 imported the repository's
30-second `crates/data/src/english_2/audio.wav` fixture and transcribed it with
the bundled Whisper Small multilingual Q8 model. The transcript was stored
locally with provider `local_whisper`; no transcript API was needed. Compared
with the fixture's checked-in reference using `eval/vi-en/score.py`, this single
English-only sample scored WER 0.080, CER 0.048, and 85.7% exact presence across
the seven listed names/terms/negations. These figures are a pipeline smoke
check, not a Vietnamese-English model evaluation.

The first simulator inference crashed while whisper.cpp allocated a Metal
buffer (`MTLSimDevice` / `_xpc_api_misuse`). `crates/whisper-local` now selects
CPU inference for the iOS Simulator target and retains Metal on physical iOS
and desktop targets. After regenerating the native bridge with the pinned Rust
1.94 toolchain and rebuilding the app, retry completed and the transcript was
persisted. The simulator run did not capture a reliable inference duration or
peak memory. It does not establish performance or stability on an actual
iPhone, nor quality on Vietnamese-English speech.
