#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

curl -sL \
  https://docs.pyannote.ai/openapi.json \
  -o openapi.gen.json

echo "Fetched openapi.gen.json ($(wc -l < openapi.gen.json) lines)"
