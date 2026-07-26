#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
crate_dir=$(cd "$script_dir/.." && pwd)
source_dir=$(mktemp -d "${TMPDIR:-/tmp}/anarlog-sqlite-sync.XXXXXX")
trap 'rm -rf "$source_dir"' EXIT

git clone --quiet https://github.com/sqliteai/sqlite-sync.git "$source_dir"
git -C "$source_dir" checkout --quiet 6b3acb5f4c7506d419e0432c7d36c993e0fdb815
git -C "$source_dir" submodule update --init --recursive --quiet
git -C "$source_dir" apply "$crate_dir/patches/sqlite-sync-1.1.2-request-deadlines.patch"
make -C "$source_dir" CPUS="${CLOUDSYNC_BUILD_CPUS:-4}" extension

for architecture in arm64 x86_64; do
  target_directory=$architecture
  if [[ "$architecture" == "arm64" ]]; then
    target_directory=aarch64
  fi
  destination="$crate_dir/vendor/cloudsync/macos/$target_directory/cloudsync.dylib"
  lipo "$source_dir/dist/cloudsync.dylib" -thin "$architecture" -output "$destination"
  chmod 755 "$destination"
done
