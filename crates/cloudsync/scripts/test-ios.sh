#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
crate_dir=$(cd "$script_dir/.." && pwd)
repo_dir=$(cd "$crate_dir/../.." && pwd)
framework="$crate_dir/vendor/cloudsync/apple/CloudSync.xcframework/ios-arm64_x86_64-simulator/CloudSync.framework/CloudSync"
test -f "$framework"

ANARLOG_CLOUDSYNC_TEST_SIMULATOR=$(xcrun simctl create "Anarlog CloudSync Tests" com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro)
export ANARLOG_CLOUDSYNC_TEST_SIMULATOR
cleanup() {
  xcrun simctl shutdown "$ANARLOG_CLOUDSYNC_TEST_SIMULATOR" >/dev/null 2>&1 || true
  xcrun simctl delete "$ANARLOG_CLOUDSYNC_TEST_SIMULATOR" || true
}
trap cleanup EXIT
xcrun simctl boot "$ANARLOG_CLOUDSYNC_TEST_SIMULATOR"
xcrun simctl bootstatus "$ANARLOG_CLOUDSYNC_TEST_SIMULATOR" -b

export SIMCTL_CHILD_CLOUDSYNC_IOS_FRAMEWORK_PATH="$framework"
export CARGO_TARGET_AARCH64_APPLE_IOS_SIM_RUNNER="$script_dir/ios-test-runner.sh"
export CARGO_TARGET_X86_64_APPLE_IOS_RUNNER="$script_dir/ios-test-runner.sh"
target=aarch64-apple-ios-sim
if [[ $(uname -m) == x86_64 ]]; then
  target=x86_64-apple-ios
fi

cd "$repo_dir"
cargo test --locked --target "$target" -p cloudsync --lib -- --test-threads=1 --skip native_http_request_deadline_is_enforced
SIMCTL_CHILD_CLOUDSYNC_CURL_CONNECT_TIMEOUT_MS=100 \
  SIMCTL_CHILD_CLOUDSYNC_CURL_TIMEOUT_MS=250 \
  cargo test --locked --target "$target" -p cloudsync --lib native_http_request_deadline_is_enforced -- --test-threads=1
cargo test --locked --target "$target" -p db-core 'cloudsync::' -- --test-threads=1
