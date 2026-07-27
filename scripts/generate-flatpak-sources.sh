#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

readonly FLATPAK_TOOLS_COMMIT="737c0085912f9f7dabf9341d4608e2a77a51a73a"
readonly REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
readonly OUTPUT_DIR="$REPO_ROOT/apps/desktop/flatpak"
readonly GENERATOR_DIR="$(mktemp -d)"

run_generator() {
  local log_file="$1"
  shift

  if ! "$@" >"$log_file" 2>&1; then
    cat "$log_file"
    return 1
  fi
}

cleanup() {
  rm -rf "$GENERATOR_DIR"
}
trap cleanup EXIT

git clone --quiet https://github.com/flatpak/flatpak-builder-tools.git "$GENERATOR_DIR/tools"
git -C "$GENERATOR_DIR/tools" checkout --quiet "$FLATPAK_TOOLS_COMMIT"
git -C "$GENERATOR_DIR/tools" apply \
  "$REPO_ROOT/scripts/patches/flatpak-cargo-generator-codeberg.patch"

python3 -m venv "$GENERATOR_DIR/venv"
"$GENERATOR_DIR/venv/bin/pip" install --quiet \
  "$GENERATOR_DIR/tools/node" \
  "$GENERATOR_DIR/tools/cargo"

cd "$REPO_ROOT"

run_generator \
  "$GENERATOR_DIR/node-generator.log" \
  "$GENERATOR_DIR/venv/bin/flatpak-node-generator" \
  pnpm \
  --node-sdk-extension org.freedesktop.Sdk.Extension.node24//25.08 \
  --pnpm-store-version v11 \
  --output "$GENERATOR_DIR/pnpm-sources.json" \
  pnpm-lock.yaml

run_generator \
  "$GENERATOR_DIR/cargo-generator.log" \
  "$GENERATOR_DIR/venv/bin/python" \
  "$GENERATOR_DIR/tools/cargo/flatpak-cargo-generator.py" \
  Cargo.lock \
  --git-tarballs \
  --output "$GENERATOR_DIR/cargo-sources.json"

# The generators emit 4-space JSON, which the repo's fmt check rejects. Python
# is already required above and matches dprint's JSON output byte for byte,
# whereas the Flatpak CI job has no Node or pnpm to run dprint with.
for sources in pnpm-sources cargo-sources; do
  python3 -m json.tool --indent 2 \
    "$GENERATOR_DIR/$sources.json" \
    "$GENERATOR_DIR/$sources.formatted.json"
  install -m 0644 \
    "$GENERATOR_DIR/$sources.formatted.json" \
    "$OUTPUT_DIR/$sources.json"
done
