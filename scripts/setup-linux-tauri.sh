#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

sudo apt update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  xdg-utils
