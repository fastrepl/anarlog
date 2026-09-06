#!/usr/bin/env bash

set -euo pipefail

case ${EAS_BUILD_PLATFORM:-} in
  ios | android) ;;
  *) exit 0 ;;
esac
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
if ! command -v rustup >/dev/null 2>&1; then
  installer=$(mktemp)
  trap 'rm -f "$installer"' EXIT
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error https://sh.rustup.rs -o "$installer"
  sh "$installer" -y --profile minimal --no-modify-path --default-toolchain none
fi

if [[ $EAS_BUILD_PLATFORM == android ]]; then
  rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
  export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-${ANDROID_HOME:-${ANDROID_SDK_ROOT:?Android SDK is required}}/ndk/27.1.12297006}}"
  test -f "$ANDROID_NDK_HOME/source.properties"
  cargo install cargo-ndk --version 4.1.2 --locked
  cargo xtask mobile-bridge android
else
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
  cargo xtask mobile-bridge ios

  # EAS installs pods before this hook, when the generated frameworks are still absent.
  cd apps/mobile/ios
  pod install
fi
