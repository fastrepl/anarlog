#!/usr/bin/env bash

set -euo pipefail

: "${ANARLOG_CLOUDSYNC_TEST_SIMULATOR:?Run test-ios.sh to select an isolated simulator}"
codesign --force --sign - "$1"
python3 - "$ANARLOG_CLOUDSYNC_TEST_SIMULATOR" "$@" <<'PY'
import subprocess
import sys

try:
    result = subprocess.run(["xcrun", "simctl", "spawn", *sys.argv[1:]], timeout=180)
    sys.exit(result.returncode)
except subprocess.TimeoutExpired:
    print("CloudSync iOS tests exceeded the watchdog", file=sys.stderr)
    sys.exit(124)
PY
