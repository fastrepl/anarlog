#!/usr/bin/env bash
# Builds char-sidecar-zoom and drops it where `sidecar2` resolves debug sidecars.
#
#   ANARLOG_ZOOM_SDK_PATH=~/Downloads/zoom-sdk-macos-7.1.5.84750/ZoomSDK ./build.sh
#
# Without ANARLOG_ZOOM_SDK_PATH the stub sidecar is built. The SDK itself is proprietary and
# is never copied into the repository; the built binary loads ZoomSDK.framework from the
# SDK directory via rpath.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
target="${TARGET:-aarch64-apple-darwin}"
resources="$repo/apps/desktop/src-tauri/resources"

swift build --package-path "$here" -c release --arch "${target%%-*}"
bin="$(swift build --package-path "$here" -c release --arch "${target%%-*}" --show-bin-path)/char-sidecar-zoom"

mkdir -p "$resources"
cp "$bin" "$resources/zoom-$target"
chmod +x "$resources/zoom-$target"
echo "installed $resources/zoom-$target"
