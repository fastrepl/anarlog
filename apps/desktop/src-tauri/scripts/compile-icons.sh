#!/bin/bash
# Compile .icon (Icon Composer) files into Assets.car using Apple's actool.
# Only runs on macOS — other platforms don't need asset catalogs.

set -euo pipefail

if [[ "$(uname)" != "Darwin" ]]; then
  echo "Skipping icon compilation (not macOS)"
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_TAURI="$(cd "$SCRIPT_DIR/.." && pwd)"
ICONS_SRC="$SRC_TAURI/icons/src"
RESOURCES="$SRC_TAURI/resources"

VARIANTS=("stable" "nightly" "staging" "pro")

for variant in "${VARIANTS[@]}"; do
  icon_path="$ICONS_SRC/${variant}.icon"
  output_dir="$RESOURCES/$variant"

  if [[ ! -d "$icon_path" ]]; then
    echo "Warning: $icon_path not found, skipping"
    continue
  fi

  mkdir -p "$output_dir"

  if [[ -f "$output_dir/Assets.car" ]]; then
    echo "Skipping $variant (Assets.car already exists)"
    continue
  fi

  echo "Compiling $variant icon..."

  tmp_dir=$(mktemp -d)
  trap "rm -rf '$tmp_dir'" EXIT

  cp -R "$icon_path" "$tmp_dir/AppIcon.icon"

  actool "$tmp_dir/AppIcon.icon" \
    --compile "$tmp_dir" \
    --output-format human-readable-text \
    --notices --warnings --errors \
    --output-partial-info-plist "$tmp_dir/assetcatalog_generated_info.plist" \
    --app-icon AppIcon \
    --include-all-app-icons \
    --enable-on-demand-resources NO \
    --target-device mac \
    --minimum-deployment-target 10.13 \
    --platform macosx

  cp "$tmp_dir/Assets.car" "$output_dir/Assets.car"
  cp "$tmp_dir/AppIcon.icns" "$output_dir/AppIcon.icns"

  rm -rf "$tmp_dir"
  trap - EXIT
done

echo "Icon compilation complete"
