#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
crate_dir=$(cd "$script_dir/.." && pwd)
build_dir=$(mktemp -d "${TMPDIR:-/tmp}/anarlog-cloudsync-ios.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT
source_dir="$build_dir/source"
destination="$crate_dir/vendor/cloudsync/apple/CloudSync.xcframework"

git clone --quiet https://github.com/sqliteai/sqlite-sync.git "$source_dir"
git -C "$source_dir" checkout --quiet 6b3acb5f4c7506d419e0432c7d36c993e0fdb815
git -C "$source_dir" submodule update --init --recursive --quiet
git -C "$source_dir" apply "$crate_dir/patches/sqlite-sync-1.1.2-request-deadlines.patch"

for platform in ios ios-sim; do
  make -C "$source_dir" clean
  make -C "$source_dir" CPUS="${CLOUDSYNC_BUILD_CPUS:-4}" PLATFORM="$platform" extension

  framework="$build_dir/$platform/CloudSync.framework"
  mkdir -p "$framework/Headers" "$framework/Modules"
  cp "$source_dir/dist/cloudsync.dylib" "$framework/CloudSync"
  cp "$source_dir/src/cloudsync.h" "$framework/Headers/CloudSync.h"
  cp "$destination/ios-arm64/CloudSync.framework/Info.plist" "$framework/Info.plist"
  cp "$destination/ios-arm64/CloudSync.framework/Modules/module.modulemap" "$framework/Modules/module.modulemap"
  install_name_tool -id '@rpath/CloudSync.framework/CloudSync' "$framework/CloudSync"
  codesign --force --sign - "$framework"
done

xcodebuild -create-xcframework \
  -framework "$build_dir/ios/CloudSync.framework" \
  -framework "$build_dir/ios-sim/CloudSync.framework" \
  -output "$build_dir/CloudSync.xcframework"
rm -rf "$destination"
mv "$build_dir/CloudSync.xcframework" "$destination"
