#!/usr/bin/env bash

set -euo pipefail

[[ ${EAS_BUILD_PLATFORM:-} == ios ]] || exit 0
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
if ! command -v rustup >/dev/null 2>&1; then
  installer=$(mktemp)
  trap 'rm -f "$installer"' EXIT
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error https://sh.rustup.rs -o "$installer"
  sh "$installer" -y --profile minimal --no-modify-path --default-toolchain none
fi

rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
cargo xtask mobile-bridge ios

# EAS installs pods before this hook, when the generated frameworks are still absent.
cd apps/mobile/ios
pod install
