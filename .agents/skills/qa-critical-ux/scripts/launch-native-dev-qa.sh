#!/usr/bin/env bash

set -euo pipefail

[[ $# -eq 1 ]] || {
  echo "Usage: $0 <app-bundle>" >&2
  exit 2
}

qa_bundle_dir="$1"
qa_open_executable="${ANARLOG_QA_OPEN_EXECUTABLE:-/usr/bin/open}"
qa_open_args=(
  -W
  --env AUDIO_SYNC_PROBE=1
  --env LISTENER_DEBUG=1
  --env NO_AEC=
)

if [[ -n "${ONBOARDING+x}" ]]; then
  qa_open_args+=(--env "ONBOARDING=$ONBOARDING")
fi

exec "$qa_open_executable" "${qa_open_args[@]}" "$qa_bundle_dir"
