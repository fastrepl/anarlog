#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
crate_dir=$(cd "$script_dir/.." && pwd)
source_dir=$(mktemp -d "${TMPDIR:-/tmp}/anarlog-sqlite-sync.XXXXXX")
trap 'rm -rf "$source_dir"' EXIT

case "$(uname -m)" in
  aarch64 | arm64)
    architecture=aarch64
    ;;
  x86_64 | amd64)
    architecture=x86_64
    ;;
  *)
    echo "Unsupported Linux architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

libc=${CLOUDSYNC_LINUX_LIBC:-}
if [[ -z "$libc" ]]; then
  ldd_version=$(ldd --version 2>&1 || true)
  if grep -qi musl <<<"$ldd_version"; then
    libc=musl
  else
    libc=gnu
  fi
fi

case "$libc" in
  gnu | musl) ;;
  *)
    echo "Unsupported Linux libc: $libc" >&2
    exit 1
    ;;
esac

git clone --quiet https://github.com/sqliteai/sqlite-sync.git "$source_dir"
git -C "$source_dir" checkout --quiet 6b3acb5f4c7506d419e0432c7d36c993e0fdb815
git -C "$source_dir" submodule update --init --recursive --quiet
git -C "$source_dir" apply "$crate_dir/patches/sqlite-sync-1.1.2-request-deadlines.patch"
make -C "$source_dir" CPUS="${CLOUDSYNC_BUILD_CPUS:-4}" extension

destination="$crate_dir/vendor/cloudsync/linux/$libc/$architecture/cloudsync.so"
install -m 755 "$source_dir/dist/cloudsync.so" "$destination"
sha256sum "$destination"
