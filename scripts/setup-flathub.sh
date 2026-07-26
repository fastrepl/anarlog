#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

sudo apt-get update
sudo apt-get install -y appstream desktop-file-utils flatpak flatpak-builder git python3-venv
flatpak remote-add --if-not-exists --user flathub https://dl.flathub.org/repo/flathub.flatpakrepo
