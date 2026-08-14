#!/bin/bash
# Bake .icon (Icon Composer) files into AppIcon.icns using Apple's actool.
# Only runs on macOS — other platforms don't need asset catalogs.
#
# The Assets.car actool also emits is intentionally discarded: shipping it makes
# macOS 26 re-render the iconstack with the Liquid Glass material, which does not
# match the flat icon the app sets on the Dock at runtime (plugins/icon).

set -euo pipefail

if [[ "$(uname)" != "Darwin" ]]; then
  echo "Skipping icon compilation (not macOS)"
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_TAURI="$(cd "$SCRIPT_DIR/.." && pwd)"
ICONS_SRC="$SRC_TAURI/icons/src"
RESOURCES="$SRC_TAURI/resources"

VARIANTS=("stable")
ARTIFACT_VARIANTS=("journal" "notepad" "stone" "typewriter-key" "walnut")

for variant in "${VARIANTS[@]}"; do
  icon_path="$ICONS_SRC/${variant}.icon"
  output_dir="$RESOURCES/$variant"

  if [[ ! -d "$icon_path" ]]; then
    echo "Warning: $icon_path not found, skipping"
    continue
  fi

  mkdir -p "$output_dir"

  if [[ -f "$output_dir/AppIcon.icns" ]]; then
    echo "Skipping $variant (AppIcon.icns already exists)"
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

  cp "$tmp_dir/AppIcon.icns" "$output_dir/AppIcon.icns"

  rm -rf "$tmp_dir"
  trap - EXIT
done

for variant in "${ARTIFACT_VARIANTS[@]}"; do
  source_image="$ICONS_SRC/anarlog-${variant}.png"
  output_dir="$RESOURCES/$variant"
  output_icon="$output_dir/AppIcon.icns"
  preview_image="$SRC_TAURI/../public/assets/app-icons/${variant}.png"

  if [[ ! -f "$source_image" ]]; then
    echo "Warning: $source_image not found, skipping"
    continue
  fi

  mkdir -p "$output_dir"

  if [[ ! -f "$output_icon" ]]; then
    echo "Compiling $variant icon..."

    tmp_dir=$(mktemp -d)
    iconset="$tmp_dir/AppIcon.iconset"
    mkdir -p "$iconset"
    trap "rm -rf '$tmp_dir'" EXIT

    for size in 16 32 128 256 512; do
      sips -z "$size" "$size" "$source_image" \
        --out "$iconset/icon_${size}x${size}.png" >/dev/null
      retina_size=$((size * 2))
      sips -z "$retina_size" "$retina_size" "$source_image" \
        --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
    done

    iconutil -c icns "$iconset" -o "$output_icon"
    rm -rf "$tmp_dir"
    trap - EXIT
  else
    echo "Skipping $variant (AppIcon.icns already exists)"
  fi

  if [[ ! -f "$preview_image" ]]; then
    echo "Generating $variant preview..."
    sips -z 128 128 "$source_image" --out "$preview_image" >/dev/null
  fi
done

echo "Icon compilation complete"
