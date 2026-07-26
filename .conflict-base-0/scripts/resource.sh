#!/bin/bash

CONFIG_FILE="$1"
RESOURCE_PATH="$2"

jq --arg res "$RESOURCE_PATH" '
 .bundle.resources = ((.bundle.resources // []) + [$res] | unique)
' "$CONFIG_FILE" > temp.json && mv temp.json "$CONFIG_FILE"

