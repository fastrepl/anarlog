#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

sudo apt update
sudo apt-get install -y \
  libgtk-3-dev \
  libgtk-4-dev \
  libasound2-dev \
  libpulse-dev \
  libgraphene-1.0-dev \
  pkg-config \
  patchelf \
  cmake \
  curl \
  libcurl4-openssl-dev

curl -fsSL https://get.pnpm.io/install.sh | sh -
