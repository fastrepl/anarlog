#!/usr/bin/env bash
set -euo pipefail

# Setup script for desktop E2E tests
# See https://v2.tauri.app/develop/tests/webdriver/#linux
# for recommended WebDriver setup instructions

# Exit if not running on Linux
if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Error: This script only supports Linux. Current OS: $(uname -s)"
  exit 1
fi

echo "Setting up desktop E2E test environment..."

# Install tauri-driver if not already installed
if ! command -v tauri-driver >/dev/null 2>&1; then
  echo "Installing tauri-driver..."
  cargo install tauri-driver --locked
else
  echo "tauri-driver already installed"
fi

# Install WebKitWebDriver package if not already installed
if ! dpkg -l | grep -q webkit2gtk-driver; then
  echo "Installing webkit2gtk-driver package..."
  sudo apt-get update
  sudo apt-get install -y webkit2gtk-driver
else
  echo "webkit2gtk-driver package already installed"
fi

# Install desktop-file-utils for update-desktop-database (required by tauri-plugin-deep-link)
if ! command -v update-desktop-database >/dev/null 2>&1; then
  echo "Installing desktop-file-utils..."
  sudo apt-get update
  sudo apt-get install -y desktop-file-utils
else
  echo "update-desktop-database already available in PATH"
fi

# Ensure WebKitWebDriver is actually callable from PATH
if ! command -v WebKitWebDriver >/dev/null 2>&1; then
  echo "WebKitWebDriver not on PATH, trying to locate binary from webkit2gtk-driver package..."
  DRIVER_PATH=$(dpkg -L webkit2gtk-driver | grep -E 'WebKitWebDriver$' | head -n 1 || true)

  if [[ -z "${DRIVER_PATH}" ]]; then
    echo "Error: Could not find WebKitWebDriver binary in webkit2gtk-driver package"
    exit 1
  fi

  echo "Found WebKitWebDriver at: ${DRIVER_PATH}"
  echo "Creating symlink at /usr/local/bin/WebKitWebDriver (requires sudo)..."
  sudo ln -sf "${DRIVER_PATH}" /usr/local/bin/WebKitWebDriver
  
  # Verify the symlink works
  if ! command -v WebKitWebDriver >/dev/null 2>&1; then
    echo "Error: WebKitWebDriver still not found in PATH after creating symlink"
    exit 1
  fi
  
  echo "WebKitWebDriver successfully linked to PATH"
else
  echo "WebKitWebDriver already available in PATH"
fi

echo "Desktop E2E test environment setup complete"
