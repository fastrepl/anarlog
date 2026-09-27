#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

case "$(uname -m)" in
  x86_64 | amd64)
    RELEASE_ARCH=amd64
    ;;
  aarch64 | arm64)
    RELEASE_ARCH=arm64
    ;;
  *)
    echo "Error: development tools support x86_64 and ARM64 Linux only." >&2
    exit 1
    ;;
esac

if ! command -v process-compose &> /dev/null; then
  PROCESS_COMPOSE_VERSION=1.122.0
  TEMP_DIR=$(mktemp -d)
  PROCESS_COMPOSE_ARCHIVE="process-compose_linux_${RELEASE_ARCH}.tar.gz"
  PROCESS_COMPOSE_URL="https://github.com/F1bonacc1/process-compose/releases/download/v${PROCESS_COMPOSE_VERSION}"
  curl -fsSL "$PROCESS_COMPOSE_URL/$PROCESS_COMPOSE_ARCHIVE" -o "$TEMP_DIR/$PROCESS_COMPOSE_ARCHIVE"
  curl -fsSL "$PROCESS_COMPOSE_URL/process-compose_checksums.txt" -o "$TEMP_DIR/checksums.txt"
  (
    cd "$TEMP_DIR"
    awk -v archive="$PROCESS_COMPOSE_ARCHIVE" '$2 == archive' checksums.txt | sha256sum --check --strict -
    tar -xzf "$PROCESS_COMPOSE_ARCHIVE"
    sudo install -m 0755 process-compose /usr/local/bin/process-compose
  )
  rm -rf "$TEMP_DIR"
fi

if [[ -x "$HOME/.dprint/bin/dprint" ]]; then
  export PATH="$HOME/.dprint/bin:$PATH"
elif ! command -v dprint &> /dev/null; then
  curl -fsSL https://dprint.dev/install.sh | sh
  export PATH="$HOME/.dprint/bin:$PATH"
fi
if [[ -x "$HOME/.dprint/bin/dprint" ]]; then
  sudo install -m 0755 "$HOME/.dprint/bin/dprint" /usr/local/bin/dprint
fi

if ! command -v supabase &> /dev/null; then
  TEMP_DIR=$(mktemp -d)
  curl -fsSL "https://github.com/supabase/cli/releases/latest/download/supabase_linux_${RELEASE_ARCH}.tar.gz" \
    | tar -xz -C "$TEMP_DIR"
  sudo install -m 0755 "$TEMP_DIR/supabase" /usr/local/bin/supabase
  rm -rf "$TEMP_DIR"
fi

if ! command -v stripe &> /dev/null; then
  curl -s https://packages.stripe.dev/api/security/keypair/stripe-cli-gpg/public | gpg --dearmor | sudo tee /usr/share/keyrings/stripe.gpg > /dev/null
  echo "deb [signed-by=/usr/share/keyrings/stripe.gpg] https://packages.stripe.dev/stripe-cli-debian-local stable main" | sudo tee /etc/apt/sources.list.d/stripe.list
  sudo apt update
  sudo apt-get install -y stripe
fi

if ! command -v task &> /dev/null; then
  curl -1sLf 'https://dl.cloudsmith.io/public/task/task/setup.deb.sh' | sudo -E bash
  sudo apt-get install -y task
fi

if ! command -v infisical &> /dev/null; then
  curl -1sLf 'https://artifacts-cli.infisical.com/setup.deb.sh' | sudo -E bash
  sudo apt-get update
  sudo apt-get install -y infisical
fi

if ! command -v dasel &> /dev/null; then
  TEMP_DIR=$(mktemp -d)
  curl -fsSL "https://github.com/TomWright/dasel/releases/latest/download/dasel_linux_${RELEASE_ARCH}" \
    -o "$TEMP_DIR/dasel"
  sudo install -m 0755 "$TEMP_DIR/dasel" /usr/local/bin/dasel
  rm -rf "$TEMP_DIR"
fi
