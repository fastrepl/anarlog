#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(dirname "$SCRIPT_DIR")
cd "$ROOT_DIR"

generated_env_file=".env.supabase"
tunnel_pid_file=".supabase/tunnel.pid"

if [ ! -f "$generated_env_file" ]; then
  echo "ERROR: $generated_env_file not found. Run 'task supabase-start' first."
  exit 1
fi

bash "$SCRIPT_DIR/supabase-tunnel-stop.sh" --quiet || true

# shellcheck disable=SC1090
source "$generated_env_file"
local_url="$SUPABASE_URL"

tunnel_output=$(mktemp)

cloudflared tunnel --url "$local_url" > "$tunnel_output" 2>&1 &
tunnel_pid=$!

public_url=""
max_wait=30
elapsed=0

while [ $elapsed -lt $max_wait ]; do
  if grep -q "https://.*\\.trycloudflare\\.com" "$tunnel_output"; then
    public_url=$(grep -o "https://[^[:space:]]*\\.trycloudflare\\.com" "$tunnel_output" | head -n1)
    break
  fi
  sleep 1
  elapsed=$((elapsed + 1))
done

if [ -z "$public_url" ]; then
  echo "ERROR: Failed to get cloudflare tunnel URL within ${max_wait}s"
  kill "$tunnel_pid" 2>/dev/null || true
  rm -f "$tunnel_output"
  exit 1
fi

echo "Cloudflare tunnel created: $public_url"
echo "Replacing $local_url with $public_url in generated env file..."

sed -i.bak "s|$local_url|$public_url|g" "$generated_env_file"
rm -f "${generated_env_file}.bak"
rm -f "$tunnel_output"

echo "Environment file updated with public URL"
echo "Tunnel PID: $tunnel_pid (running in background)"
echo "$tunnel_pid" > "$tunnel_pid_file"

