#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(dirname "$SCRIPT_DIR")
cd "$ROOT_DIR"

quiet=0
if [ "${1:-}" = "--quiet" ]; then
  quiet=1
fi

pid_file=".supabase/tunnel.pid"

log() {
  if [ "$quiet" -eq 0 ]; then
    echo "$@"
  fi
}

if [ ! -f "$pid_file" ]; then
  log "No Cloudflare tunnel to stop."
  exit 0
fi

pid=$(cat "$pid_file")

if kill -0 "$pid" 2>/dev/null; then
  log "Stopping Cloudflare tunnel (PID $pid)..."
  kill "$pid" 2>/dev/null || true
  sleep 1
  if kill -0 "$pid" 2>/dev/null; then
    log "Tunnel still running, forcing termination..."
    kill -9 "$pid" 2>/dev/null || true
  fi
else
  log "Cloudflare tunnel process $pid not running."
fi

rm -f "$pid_file"
log "Cloudflare tunnel stopped."
