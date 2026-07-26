#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

curl -sL \
  https://raw.githubusercontent.com/chatwoot/chatwoot/refs/heads/develop/swagger/swagger.json \
  -o swagger.gen.json

echo "Fetched swagger.gen.json ($(wc -l < swagger.gen.json) lines)"
