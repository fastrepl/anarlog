#!/bin/bash

CONFIG_FILE="$1"
VERSION="$2"

jq --arg ver "$VERSION" '
  .version = $ver
' "$CONFIG_FILE" > temp.json && mv temp.json "$CONFIG_FILE"

