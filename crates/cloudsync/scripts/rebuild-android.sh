#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
crate_dir=$(cd "$script_dir/.." && pwd)
build_dir=$(mktemp -d "${TMPDIR:-/tmp}/anarlog-cloudsync-android.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT
source_dir="$build_dir/source"
export ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to the installed Android NDK}}"
export ANDROID_NDK="$ANDROID_NDK_ROOT"
test -f "$ANDROID_NDK/source.properties"

git clone --quiet https://github.com/sqliteai/sqlite-sync.git "$source_dir"
git -C "$source_dir" checkout --quiet 6b3acb5f4c7506d419e0432c7d36c993e0fdb815
git -C "$source_dir" submodule update --init --recursive --quiet
git -C "$source_dir" apply "$crate_dir/patches/sqlite-sync-1.1.2-request-deadlines.patch"

for architecture in arm64-v8a armeabi-v7a x86_64; do
  make -C "$source_dir" clean
  make -C "$source_dir" CPUS="${CLOUDSYNC_BUILD_CPUS:-4}" PLATFORM=android ARCH="$architecture" extension
  destination="$crate_dir/vendor/cloudsync/android/$architecture/cloudsync.so"
  install -m 755 "$source_dir/dist/cloudsync.so" "$destination"
  shasum -a 256 "$destination"
done
