#!/bin/bash
# Compile the Mac App Store Assets.car from the stable icns.
# Direct-distribution builds must not ship this catalog; compile-icons.sh
# discards it so macOS 26 does not Liquid-Glass the Dock icon.

set -euo pipefail

if [[ "$(uname)" != "Darwin" ]]; then
  echo "::error::Mac App Store asset catalog compilation requires macOS"
  exit 1
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
tmp_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
tmp_root="${tmp_root%/}"
asset_output="$tmp_root/mac-app-store-assets"
asset_source="$tmp_root/AnarlogAssets.xcassets"
extracted_iconset="$tmp_root/AppIcon.iconset"
iconset="$asset_source/AppIcon.appiconset"
asset_catalog="$repo_root/apps/desktop/src-tauri/resources/app-store/Assets.car"
icon_icns="$repo_root/apps/desktop/src-tauri/icons/stable/icon.icns"

rm -rf "$asset_output" "$asset_source" "$extracted_iconset"
mkdir -p "$asset_output" "$asset_source" "$(dirname "$asset_catalog")"

iconutil \
  --convert iconset \
  --output "$extracted_iconset" \
  "$icon_icns"
mv "$extracted_iconset" "$iconset"

cat > "$iconset/Contents.json" <<'JSON'
{
  "images": [
    { "filename": "icon_16x16.png", "idiom": "mac", "scale": "1x", "size": "16x16" },
    { "filename": "icon_16x16@2x.png", "idiom": "mac", "scale": "2x", "size": "16x16" },
    { "filename": "icon_32x32.png", "idiom": "mac", "scale": "1x", "size": "32x32" },
    { "filename": "icon_32x32@2x.png", "idiom": "mac", "scale": "2x", "size": "32x32" },
    { "filename": "icon_128x128.png", "idiom": "mac", "scale": "1x", "size": "128x128" },
    { "filename": "icon_128x128@2x.png", "idiom": "mac", "scale": "2x", "size": "128x128" },
    { "filename": "icon_256x256.png", "idiom": "mac", "scale": "1x", "size": "256x256" },
    { "filename": "icon_256x256@2x.png", "idiom": "mac", "scale": "2x", "size": "256x256" },
    { "filename": "icon_512x512.png", "idiom": "mac", "scale": "1x", "size": "512x512" },
    { "filename": "icon_512x512@2x.png", "idiom": "mac", "scale": "2x", "size": "512x512" }
  ],
  "info": { "author": "xcode", "version": 1 }
}
JSON

xcrun actool \
  "$asset_source" \
  --compile "$asset_output" \
  --output-format human-readable-text \
  --notices \
  --warnings \
  --errors \
  --output-partial-info-plist "$asset_output/assetcatalog_generated_info.plist" \
  --app-icon AppIcon \
  --include-all-app-icons \
  --enable-on-demand-resources NO \
  --target-device mac \
  --minimum-deployment-target 10.13 \
  --platform macosx

if [[ ! -s "$asset_output/Assets.car" ]]; then
  echo "::error::actool did not produce the Mac App Store asset catalog"
  exit 1
fi

cp "$asset_output/Assets.car" "$asset_catalog"
echo "Wrote $asset_catalog"
