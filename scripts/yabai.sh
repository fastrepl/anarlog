#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

app_char=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --app-char)
      app_char="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [[ -n "$app_char" ]]; then
  "$SCRIPT_DIR/yabai_impl.sh" --bundle-id "$app_char" --position left
fi
