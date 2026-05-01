#!/usr/bin/env bash
set -euo pipefail

repo=${CLAW_REPO:-https://github.com/fastrepl/char}
ref=${CLAW_REF:-main}

if ! command -v cargo >/dev/null 2>&1; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
fi

source "${HOME}/.cargo/env"

cargo install \
  --git "${repo}" \
  --branch "${ref}" \
  --locked \
  --bin claw \
  --force

npm install -g @googleworkspace/cli
npm install -g char
