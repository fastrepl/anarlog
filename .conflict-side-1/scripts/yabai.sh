#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

app_hyprnote=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --app-hyprnote)
      app_hyprnote="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [[ -n "$app_hyprnote" ]]; then
  "$SCRIPT_DIR/yabai_impl.sh" --bundle-id "$app_hyprnote" --position left
fi
