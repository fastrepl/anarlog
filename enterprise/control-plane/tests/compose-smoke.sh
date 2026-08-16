#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compose_file="$script_dir/../compose.yaml"
project_name="anarlog-enterprise-smoke-$$"
environment_file=$(mktemp "${TMPDIR:-/tmp}/anarlog-enterprise-smoke.XXXXXX")
token=0123456789abcdef0123456789abcdef

cleanup() {
    docker compose \
        --file "$compose_file" \
        --project-name "$project_name" \
        --env-file "$environment_file" \
        down --volumes --remove-orphans >/dev/null 2>&1 || true
    rm -f "$environment_file"
}
trap cleanup EXIT HUP INT TERM

cat >"$environment_file" <<EOF
POSTGRES_DB=anarlog
POSTGRES_USER=anarlog
POSTGRES_PASSWORD=0123456789abcdef0123456789abcdef
ANARLOG_ENTERPRISE_DATABASE_URL=postgres://anarlog:0123456789abcdef0123456789abcdef@postgres:5432/anarlog
ANARLOG_ENTERPRISE_DATABASE_MAX_CONNECTIONS=2
ANARLOG_ENTERPRISE_WORKSPACE_TOKENS={"workspace-a":"$token"}
ANARLOG_ENTERPRISE_PORT=0
RUST_LOG=info
EOF

docker compose \
    --file "$compose_file" \
    --project-name "$project_name" \
    --env-file "$environment_file" \
    up --build --wait --detach

published=$(docker compose \
    --file "$compose_file" \
    --project-name "$project_name" \
    --env-file "$environment_file" \
    port control-plane 8080)
port=${published##*:}
base_url="http://127.0.0.1:$port"

curl --connect-timeout 3 --max-time 10 --fail --silent --show-error "$base_url/health/ready" >/dev/null

authorized_status=$(curl \
    --connect-timeout 3 \
    --max-time 10 \
    --silent \
    --output /dev/null \
    --write-out '%{http_code}' \
    --header "Authorization: Bearer $token" \
    "$base_url/v1/workspaces/workspace-a/session-envelopes?consumerId=device-a&after=0")
test "$authorized_status" = 200

forbidden_status=$(curl \
    --connect-timeout 3 \
    --max-time 10 \
    --silent \
    --output /dev/null \
    --write-out '%{http_code}' \
    --header "Authorization: Bearer $token" \
    "$base_url/v1/workspaces/workspace-b/session-envelopes?consumerId=device-a&after=0")
test "$forbidden_status" = 403
