#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
crate_dir=$(cd "$script_dir/.." && pwd)
source_dir=$(mktemp -d "${TMPDIR:-/tmp}/anarlog-sqlite-sync.XXXXXX")
trap 'rm -rf "$source_dir"' EXIT

if [[ "$(uname -m)" != "x86_64" ]]; then
  echo "Windows CloudSync builds currently require x86_64, got $(uname -m)" >&2
  exit 1
fi

case "${MSYSTEM:-}" in
  UCRT64) ;;
  *)
    echo "Run this script from an x86_64 MSYS2 UCRT64 shell" >&2
    exit 1
    ;;
esac

git clone --quiet https://github.com/sqliteai/sqlite-sync.git "$source_dir"
git -C "$source_dir" checkout --quiet 285a1390c92fce3a1dc6f98daef213f882a5b23b
git -C "$source_dir" submodule update --init --recursive --quiet
make -C "$source_dir" \
  PLATFORM=windows \
  HOST=windows \
  CPUS="${CLOUDSYNC_BUILD_CPUS:-4}" \
  extension

destination="$crate_dir/vendor/cloudsync/windows/x86_64/cloudsync.dll"
install -m 755 "$source_dir/dist/cloudsync.dll" "$destination"
sha256sum "$destination"
