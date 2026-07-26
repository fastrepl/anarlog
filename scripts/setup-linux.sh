#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NODE_MAJOR=22
RUST_VERSION=1.94.0
PNPM_VERSION=11.1.1

setup_toolchains() {
  if ! command -v apt-get &> /dev/null || [[ ! -f /etc/os-release ]]; then
    echo "Error: Linux setup currently supports Debian and Ubuntu." >&2
    exit 1
  fi

  . /etc/os-release
  case "${ID:-}" in
    debian | ubuntu) ;;
    *)
      echo "Error: Linux setup currently supports Debian and Ubuntu, not ${ID:-unknown}." >&2
      exit 1
      ;;
  esac

  case "$(uname -m)" in
    x86_64 | amd64 | aarch64 | arm64) ;;
    *)
      echo "Error: Linux setup supports x86_64 and ARM64 only." >&2
      exit 1
      ;;
  esac

  sudo apt-get update
  sudo apt-get install -y ca-certificates curl libatomic1

  local installed_node_major=0
  if command -v node &> /dev/null; then
    installed_node_major="$(node --version)"
    installed_node_major="${installed_node_major#v}"
    installed_node_major="${installed_node_major%%.*}"
  fi

  if ((installed_node_major < NODE_MAJOR)); then
    local nodesource_setup
    nodesource_setup="$(mktemp)"
    curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" -o "$nodesource_setup"
    sudo -E bash "$nodesource_setup"
    rm -f "$nodesource_setup"
    sudo apt-get install -y nodejs
  fi

  local cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  if [[ -f "$cargo_home/env" ]]; then
    . "$cargo_home/env"
  else
    export PATH="$cargo_home/bin:$PATH"
  fi
  if ! command -v rustup &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain "$RUST_VERSION"
  fi
  rustup toolchain install "$RUST_VERSION" --profile minimal

  export PNPM_HOME="${PNPM_HOME:-$HOME/.local/share/pnpm}"
  export PATH="$PNPM_HOME/bin:$PATH"
  if ! command -v pnpm &> /dev/null || [[ "$(pnpm --version)" != "$PNPM_VERSION" ]]; then
    curl -fsSL https://get.pnpm.io/install.sh \
      | env PNPM_VERSION="$PNPM_VERSION" SHELL="${SHELL:-/bin/bash}" sh -
  fi

  echo "Node $(node --version), Rust $(rustc --version), pnpm $(pnpm --version)"
}

if [[ "$OSTYPE" == "linux-gnu"* ]]; then
  setup_toolchains
  bash "$SCRIPT_DIR/setup-linux-others.sh"
  bash "$SCRIPT_DIR/setup-linux-tauri.sh"
  bash "$SCRIPT_DIR/setup-devtools.sh"
elif [[ "$OSTYPE" == "darwin"* ]]; then
  echo "Error: macOS is not supported by this script"
  exit 1
else
  echo "Error: Unsupported OS: $OSTYPE"
  exit 1
fi
