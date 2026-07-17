#!/usr/bin/env bash
set -euo pipefail

# Fixes dynamic library linking in Tauri app bundles.
# Workaround for https://github.com/tauri-apps/tauri/pull/12711
#
# - macOS: rewrites .dylib paths to use @rpath so the app finds bundled libs
# - Linux: sets RPATH to $ORIGIN so the binary finds .so files next to it
# - Windows: no-op (DLLs next to .exe are found automatically)
#
# This runs as Tauri's beforeBundleCommand. Safe to run when no dylibs are present.

OS="$(uname -s)"

case "$OS" in
  Darwin)
    BUNDLE_DIR="${1:-}"
    if [ -z "$BUNDLE_DIR" ]; then
      echo "[fix-dylib] No bundle dir provided, skipping."
      exit 0
    fi

    APP_BUNDLE=$(find "$BUNDLE_DIR" -name "*.app" -maxdepth 1 | head -n 1)
    if [ -z "$APP_BUNDLE" ]; then
      echo "[fix-dylib] No .app bundle found, skipping."
      exit 0
    fi

    FRAMEWORKS="$APP_BUNDLE/Contents/Frameworks"
    if [ ! -d "$FRAMEWORKS" ]; then
      echo "[fix-dylib] No Frameworks directory, skipping."
      exit 0
    fi

    DYLIBS=$(find "$FRAMEWORKS" -name "*.dylib" 2>/dev/null)
    if [ -z "$DYLIBS" ]; then
      echo "[fix-dylib] No dylibs found, skipping."
      exit 0
    fi

    BINARY_NAME=$(defaults read "$APP_BUNDLE/Contents/Info.plist" CFBundleExecutable)
    BINARY="$APP_BUNDLE/Contents/MacOS/$BINARY_NAME"

    if [ ! -f "$BINARY" ]; then
      echo "[fix-dylib] Binary not found: $BINARY"
      exit 1
    fi

    echo "[fix-dylib] Fixing dylib paths in: $APP_BUNDLE"

    # Add rpath if not already present
    if ! otool -l "$BINARY" | grep -q "@executable_path/../Frameworks"; then
      install_name_tool -add_rpath "@executable_path/../Frameworks" "$BINARY"
    fi

    for dylib_path in $DYLIBS; do
      dylib=$(basename "$dylib_path")

      # Fix the dylib's own install name
      install_name_tool -id "@rpath/$dylib" "$dylib_path"

      # Rewrite the binary's reference to this dylib (try common prefixes)
      for prefix in /usr/local/lib /opt/homebrew/lib /opt/homebrew/opt/*/lib; do
        install_name_tool -change "$prefix/$dylib" "@rpath/$dylib" "$BINARY" 2>/dev/null || true
      done

      # Also fix inter-dylib references
      for other_path in $DYLIBS; do
        other=$(basename "$other_path")
        if [ "$dylib" != "$other" ]; then
          for prefix in /usr/local/lib /opt/homebrew/lib /opt/homebrew/opt/*/lib; do
            install_name_tool -change "$prefix/$other" "@rpath/$other" "$dylib_path" 2>/dev/null || true
          done
        fi
      done

      echo "[fix-dylib]   Fixed: $dylib"
    done

    echo "[fix-dylib] Done."
    ;;

  Linux)
    BINARY="${1:-}"
    if [ -z "$BINARY" ] || [ ! -f "$BINARY" ]; then
      echo "[fix-dylib] No binary provided or not found, skipping."
      exit 0
    fi

    if ! command -v patchelf &>/dev/null; then
      echo "[fix-dylib] patchelf not found, skipping."
      exit 0
    fi

    BINARY_DIR=$(dirname "$BINARY")
    SO_FILES=$(find "$BINARY_DIR" -name "*.so*" 2>/dev/null)
    if [ -z "$SO_FILES" ]; then
      echo "[fix-dylib] No .so files found, skipping."
      exit 0
    fi

    echo "[fix-dylib] Setting RPATH on: $BINARY"
    patchelf --set-rpath '$ORIGIN' "$BINARY"
    echo "[fix-dylib] Done."
    ;;

  *)
    echo "[fix-dylib] Windows or unknown OS, no action needed."
    ;;
esac
