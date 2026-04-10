#!/bin/bash

if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "This script only works on macOS"
    exit 1
fi

if pgrep -q "Char"; then
    echo "ERROR: Char apps are still running. Please quit them first."
    exit 1
fi

STABLE_DIR="$HOME/Library/Application Support/char"
NIGHTLY_DIR="$HOME/Library/Application Support/char-nightly"
TEMP_DIR=$(mktemp -d)

if [[ ! -d "$STABLE_DIR" ]]; then
    echo "ERROR: Stable directory not found: $STABLE_DIR"
    exit 1
fi

if [[ ! -d "$NIGHTLY_DIR" ]]; then
    echo "ERROR: Nightly directory not found: $NIGHTLY_DIR"
    exit 1
fi

if ! cp -r "$STABLE_DIR" ./backup_stable; then
    echo "ERROR: Failed to backup stable directory"
    exit 1
fi

if ! cp -r "$NIGHTLY_DIR" ./backup_nightly; then
    echo "ERROR: Failed to backup nightly directory"
    exit 1
fi

if ! mv "$STABLE_DIR" "$TEMP_DIR"; then
    echo "ERROR: Failed to move stable directory"
    exit 1
fi

if ! mv "$NIGHTLY_DIR" "$STABLE_DIR"; then
    echo "ERROR: Failed to move nightly directory"
    mv "$TEMP_DIR" "$STABLE_DIR"
    exit 1
fi

if ! mv "$TEMP_DIR" "$NIGHTLY_DIR"; then
    echo "ERROR: Failed to move temp to nightly"
    mv "$STABLE_DIR" "$NIGHTLY_DIR"
    mv "$TEMP_DIR" "$STABLE_DIR"
    exit 1
fi

if [[ -d "$STABLE_DIR" && -d "$NIGHTLY_DIR" ]]; then
    rm -rf ./backup_stable ./backup_nightly
    echo "Swap completed successfully"
else
    echo "ERROR: Verification failed. Backups preserved in:"
    echo "  - ./backup_stable"
    echo "  - ./backup_nightly"
    exit 1
fi
