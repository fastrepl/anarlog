#!/usr/bin/env bash

set -euo pipefail

[[ $# -eq 0 ]] || {
  echo "Usage: $0" >&2
  exit 2
}

[[ "$(uname -s)" == "Darwin" ]] || {
  echo "Staging QA permission reset requires macOS." >&2
  exit 1
}

qa_process_status=0
pgrep -x "anarlog-staging" >/dev/null || qa_process_status=$?
case "$qa_process_status" in
  0)
    echo "Quit BlackMushi Staging before resetting permissions." >&2
    exit 1
    ;;
  1) ;;
  *)
    echo "Could not determine whether BlackMushi Staging is running." >&2
    exit 1
    ;;
esac

# Reset before LaunchServices starts the process that will request permission.
for qa_service in Microphone AudioCapture ScreenCapture Accessibility Calendar Reminders; do
  tccutil reset "$qa_service" com.hyprnote.staging
done

echo "Permissions reset for com.hyprnote.staging. Launch normally, without ONBOARDING."
