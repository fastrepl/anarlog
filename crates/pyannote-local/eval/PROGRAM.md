# Local diarization autoresearch loop

You are improving the on-device speaker diarizer in `crates/pyannote-local`.
The metric is diarization error rate (DER, collar 0.25 s) on the dev set;
lower is better. Work like a ratchet: one change, one measurement, keep or
revert.

## Setup (once)

```bash
uv run crates/pyannote-local/eval/fetch_datasets.py --out /tmp/diar-dev \
    --dataset diarizers-community/ami:validation --dataset diarizers-community/voxconverse:validation
uv run crates/pyannote-local/eval/fetch_datasets.py --out /tmp/diar-test \
    --dataset diarizers-community/ami:test --dataset diarizers-community/voxconverse:test
cargo run --release -p pyannote-local --features eval --bin diarization-eval -- /tmp/diar-dev
```

Record the baseline `DER=` line in `results.tsv` (columns: commit, description,
DER, rtf). Never run on `/tmp/diar-test` until the loop is finished.

## Files you may edit

- `crates/pyannote-local/src/pipeline.rs` — windowing, masking, voting,
  reconstruction, `DiarizationConfig::default()`.
- `crates/pyannote-local/src/clustering.rs` — linkage, cut selection,
  small-cluster handling.

Do not edit `metrics.rs`, `segmentation.rs`, the eval binary, or the models.
Keep the public API (`Diarizer`, `DiarizeRequest`, `Diarization`,
`assign_words`) unchanged so the batch pipeline keeps compiling.

## Loop

1. Read the last rows of `results.tsv` and the current code. Propose one
   change. Prefer ideas that address the dominant error component (miss,
   false alarm, or confusion) reported per file.
2. Edit, then `cargo test -p pyannote-local`. A failing test means revert.
3. `git commit -am "<idea>"`.
4. Run the eval on `/tmp/diar-dev`; append a row to `results.tsv` with the
   `DER=` value and `rtf`.
5. Keep the commit if DER improved by more than 0.1 absolute points and
   `rtf` stayed under 0.15 on the reference machine; otherwise
   `git reset --hard HEAD~1`.
6. Repeat.

## Ideas worth trying

- Soft aggregation (average powerset probabilities across windows) instead
  of binary votes; onset/offset hysteresis on the aggregated track.
- Clustering threshold sweeps; centroid vs average linkage; using only
  anchors with more clean speech; weighting anchors by clean duration.
- Reassigning weak (short) window speakers by frame-level similarity
  instead of nearest centroid.
- `min_duration_on` / `min_duration_off` post-processing.
- Adaptive step: 1 s for recordings under 20 min, growing for longer ones.
- Using `known_speakers` more aggressively when they match confidently.

Report the final dev and test DER, the kept commits, and the rejected ideas.
