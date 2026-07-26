#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ]; then
  echo "Error: this script must be run with bash, not sh." >&2
  echo "Try: bash \"$0\"" >&2
  exit 1
fi
