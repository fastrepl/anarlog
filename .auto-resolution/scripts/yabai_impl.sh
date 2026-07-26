#!/bin/bash

bundle_id=""
position="left"

while [[ $# -gt 0 ]]; do
  case $1 in
    --bundle-id)
      bundle_id="$2"
      shift 2
      ;;
    --position)
      position="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [[ -z "$bundle_id" ]]; then
  echo "Error: --bundle-id is required"
  exit 1
fi

if [[ "$position" != "left" && "$position" != "right" ]]; then
  echo "Error: --position must be 'left' or 'right'"
  exit 1
fi

pid=$(pgrep -f "$bundle_id" 2>/dev/null | head -1)
if [[ -z "$pid" ]]; then
  echo "Error: No process found for bundle ID '$bundle_id'"
  exit 1
fi

window_id=$(yabai -m query --windows | jq -r --arg pid "$pid" 'map(select(.pid == ($pid | tonumber))) | .[0].id // empty')
if [[ -z "$window_id" ]]; then
  echo "Error: No window found for bundle ID '$bundle_id'"
  exit 1
fi

yabai -m window --focus "$window_id" 2>/dev/null

grid_x=$([[ "$position" == "left" ]] && echo "0" || echo "1")
yabai -m window "$window_id" --grid 1:2:$grid_x:0:1:1
