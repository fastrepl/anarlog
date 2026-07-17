#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "$OSTYPE" == "linux-gnu"* ]]; then
  bash "$SCRIPT_DIR/setup-linux-others.sh"
  bash "$SCRIPT_DIR/setup-linux-tauri.sh"
  bash "$SCRIPT_DIR/setup-devtools.sh"
elif [[ "$OSTYPE" == "darwin"* ]]; then
  echo "Error: macOS is not supported by this script"
  exit 1
else
  echo "Error: Unsupported OS: $OSTYPE"
  exit 1
fi
