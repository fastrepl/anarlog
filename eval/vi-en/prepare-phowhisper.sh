#!/usr/bin/env bash
set -euo pipefail

WORK_DIR="${1:-${TMPDIR:-/tmp}/anarlog-phowhisper-eval}"
PHO_REVISION="a86b604c346caf7148c37512eafe783a16420adb"
PHO_WEIGHTS_SHA256="9ec6f07c5bd321fc8f477cf778c43c3fd94b0e33a9f20283fc61cc6d22cbfded"
WHISPER_CPP_REVISION="2eeeba56e9edd762b4b38467bab96c2517163158"
OPENAI_WHISPER_REVISION="86098128c0b4f24f0e2aa2994de830614b474227"

for command in git hf python3 cmake; do
  command -v "$command" >/dev/null || {
    echo "Required command not found: $command" >&2
    exit 1
  }
done
python3 -c 'import numpy, torch, transformers' || {
  echo "Install Python packages numpy, torch, and transformers in a dedicated environment first." >&2
  exit 1
}

mkdir -p "$WORK_DIR"
MODEL_DIR="$WORK_DIR/PhoWhisper-small"
CPP_DIR="$WORK_DIR/whisper.cpp"
OPENAI_DIR="$WORK_DIR/openai-whisper"
BUILD_DIR="$WORK_DIR/build"
OUTPUT_DIR="$WORK_DIR/output"

if [[ ! -d "$CPP_DIR/.git" ]]; then
  git clone https://github.com/ggml-org/whisper.cpp.git "$CPP_DIR"
fi
git -C "$CPP_DIR" fetch --depth 1 origin "$WHISPER_CPP_REVISION"
git -C "$CPP_DIR" checkout --detach "$WHISPER_CPP_REVISION"

if [[ ! -d "$OPENAI_DIR/.git" ]]; then
  git clone https://github.com/openai/whisper.git "$OPENAI_DIR"
fi
git -C "$OPENAI_DIR" fetch --depth 1 origin "$OPENAI_WHISPER_REVISION"
git -C "$OPENAI_DIR" checkout --detach "$OPENAI_WHISPER_REVISION"

hf download vinai/PhoWhisper-small --revision "$PHO_REVISION" --local-dir "$MODEL_DIR"
WEIGHTS="$MODEL_DIR/pytorch_model.bin"
if command -v shasum >/dev/null; then
  ACTUAL_SHA="$(shasum -a 256 "$WEIGHTS" | awk '{print $1}')"
else
  ACTUAL_SHA="$(sha256sum "$WEIGHTS" | awk '{print $1}')"
fi
if [[ "$ACTUAL_SHA" != "$PHO_WEIGHTS_SHA256" ]]; then
  echo "PhoWhisper checkpoint checksum mismatch: $ACTUAL_SHA" >&2
  exit 1
fi

cmake -S "$CPP_DIR" -B "$BUILD_DIR" \
  -DWHISPER_BUILD_TESTS=OFF \
  -DWHISPER_BUILD_EXAMPLES=ON \
  -DWHISPER_BUILD_SERVER=OFF
cmake --build "$BUILD_DIR" --target whisper-cli whisper-quantize --parallel 6

mkdir -p "$OUTPUT_DIR/f16"
python3 "$CPP_DIR/models/convert-h5-to-ggml.py" \
  "$MODEL_DIR" "$OPENAI_DIR" "$OUTPUT_DIR/f16"
mv "$OUTPUT_DIR/f16/ggml-model.bin" "$OUTPUT_DIR/pho-whisper-small-f16.bin"
"$BUILD_DIR/bin/whisper-quantize" \
  "$OUTPUT_DIR/pho-whisper-small-f16.bin" \
  "$OUTPUT_DIR/pho-whisper-small-q8_0.bin" q8_0

# Run a tiny upstream fixture through both artifacts to prove the files load.
"$BUILD_DIR/bin/whisper-cli" -m "$OUTPUT_DIR/pho-whisper-small-f16.bin" \
  -f "$CPP_DIR/samples/jfk.wav" -l en -np >/dev/null
"$BUILD_DIR/bin/whisper-cli" -m "$OUTPUT_DIR/pho-whisper-small-q8_0.bin" \
  -f "$CPP_DIR/samples/jfk.wav" -l en -np >/dev/null

echo "Created and load-checked local candidates in: $OUTPUT_DIR"
echo "Do not publish these artifacts until corpus quality and runtime are evaluated."
