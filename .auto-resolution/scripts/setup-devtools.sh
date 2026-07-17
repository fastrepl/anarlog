#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

if ! command -v dprint &> /dev/null; then
  curl -fsSL https://dprint.dev/install.sh | sh
  export PATH="$HOME/.dprint/bin:$PATH"
fi

if ! command -v supabase &> /dev/null; then
  TEMP_DIR=$(mktemp -d)
  curl -fsSL https://github.com/supabase/cli/releases/latest/download/supabase_linux_amd64.tar.gz | tar -xz -C "$TEMP_DIR"
  sudo mv "$TEMP_DIR/supabase" /usr/local/bin/supabase
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
  curl -sSLf "$(curl -sSLf https://api.github.com/repos/tomwright/dasel/releases/latest | grep browser_download_url | grep linux_amd64 | grep -v .gz | cut -d\" -f 4)" -L -o dasel && chmod +x dasel
  sudo mv ./dasel /usr/local/bin/dasel
fi
