#!/bin/bash

CONFIG_FILE="$1"
BINARY_PATH="$2"

jq --arg bin "$BINARY_PATH" '
 .bundle.externalBin = ((.bundle.externalBin // []) + [$bin] | unique)
' "$CONFIG_FILE" > temp.json && mv temp.json "$CONFIG_FILE"
