#!/bin/bash
set -euo pipefail

# Builds the macOS desktop bundle and makes sure it ends up with a real
# signature. Without one, `tauri build` leaves the linker's ad-hoc signature in
# place: no entitlements, and a code-signing identifier derived from the build
# hash instead of the bundle id. TCC then keys the app on its cdhash, so every
# rebuild loses the permissions the user already granted.
#
# Usage:
#   scripts/build-macos.sh [tauri build args...]
#   scripts/build-macos.sh --sign-only <path to .app>   # re-sign, no rebuild
#
# Xcode 26+ ships a SwiftPM build system that internalizes `@_cdecl` symbols in
# release, which breaks swift-rs linking, so pin the Command Line Tools toolchain.
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Library/Developer/CommandLineTools}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP_DIR="$REPO_ROOT/apps/desktop"

resolve_identity() {
  if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
    printf '%s' "$APPLE_SIGNING_IDENTITY"
    return
  fi

  security find-identity -v -p codesigning 2>/dev/null |
    sed -n 's/.*"\(Developer ID Application:.*\)".*/\1/p' |
    head -n 1
}

# Signs the bundle in place. A Developer ID keeps its designated requirement
# stable across rebuilds, which is what lets TCC recognise the app again.
sign_app() {
  local app="$1"
  local identity="$2"
  local entitlements="$3"
  local info_plist="$app/Contents/Info.plist"
  local identifier executable main_binary
  local options=()

  identifier="$(/usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$info_plist")"
  executable="$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$info_plist")"
  main_binary="$app/Contents/MacOS/$executable"

  # The hardened runtime requires library validation, which an ad-hoc signature
  # cannot satisfy for the vendored dylib.
  if [[ "$identity" != "-" ]]; then
    options=(--timestamp --options runtime)
  fi

  # Nested Mach-O first: the bundle signature seals them, so signing them
  # afterwards would invalidate the outer seal.
  while IFS= read -r -d '' nested; do
    if [[ "$nested" == "$main_binary" ]]; then
      continue
    fi
    if [[ "$(file -b --mime-type "$nested")" != "application/x-mach-binary" ]]; then
      continue
    fi
    codesign --force --sign "$identity" "${options[@]+"${options[@]}"}" "$nested"
  done < <(find "$app/Contents" -type f -print0)

  codesign --force --sign "$identity" \
    --identifier "$identifier" \
    --entitlements "$entitlements" \
    "${options[@]+"${options[@]}"}" \
    "$app"
  codesign --verify --verbose=2 "$app"
}

entitlements_for() {
  if [[ "$*" == *app-store* ]]; then
    printf '%s' "$DESKTOP_DIR/src-tauri/Entitlements.app-store.plist"
  else
    printf '%s' "$DESKTOP_DIR/src-tauri/Entitlements.plist"
  fi
}

warn_adhoc() {
  local app="$1"

  cat >&2 <<EOF

No Developer ID certificate found, so $(basename "$app") was signed ad-hoc.
Entitlements and the bundle identifier are now correct, but macOS identifies an
ad-hoc app by its cdhash: the permissions granted to this build will not carry
over to the next one. Install a Developer ID Application certificate (or set
APPLE_SIGNING_IDENTITY) to keep granted permissions across rebuilds.
EOF
}

signing_identity="$(resolve_identity)"
[[ -n "$signing_identity" ]] || signing_identity="-"

if [[ "${1:-}" == "--sign-only" ]]; then
  app_bundle="${2:-}"
  if [[ -z "$app_bundle" || ! -d "$app_bundle" ]]; then
    echo "Usage: $0 --sign-only <path to .app>" >&2
    exit 1
  fi

  sign_app "$app_bundle" "$signing_identity" "$(entitlements_for "$app_bundle")"
  if [[ "$signing_identity" == "-" ]]; then
    warn_adhoc "$app_bundle"
  fi
  exit 0
fi

target=""
previous=""
for argument in "$@"; do
  if [[ "$previous" == "--target" ]]; then
    target="$argument"
  fi
  previous="$argument"
done

if [[ -n "$target" ]]; then
  bundle_dir="$DESKTOP_DIR/src-tauri/target/$target/release/bundle/macos"
else
  bundle_dir="$DESKTOP_DIR/src-tauri/target/release/bundle/macos"
  case "$(uname -m)" in
    arm64) target="aarch64-apple-darwin" ;;
    *) target="x86_64-apple-darwin" ;;
  esac
fi

if [[ "$signing_identity" != "-" ]]; then
  echo "Signing with: $signing_identity"

  # Tauri copies the vendored dylib in as a framework, and the hardened runtime
  # refuses to load one that a different identity signed. It is checked into the
  # repository, so put the original back once the bundle holds a signed copy.
  cloudsync_dylib="$REPO_ROOT/crates/cloudsync/vendor/cloudsync/macos/${target%%-*}/cloudsync.dylib"
  if [[ -f "$cloudsync_dylib" ]]; then
    cloudsync_backup="$(mktemp -t cloudsync)"
    cp -p "$cloudsync_dylib" "$cloudsync_backup"
    # shellcheck disable=SC2064
    trap "cp -p '$cloudsync_backup' '$cloudsync_dylib'; rm -f '$cloudsync_backup'" EXIT

    codesign --force --sign "$signing_identity" --timestamp --options runtime "$cloudsync_dylib"
    codesign --verify --strict --verbose=2 "$cloudsync_dylib"
  fi

  export APPLE_SIGNING_IDENTITY="$signing_identity"
fi

cd "$DESKTOP_DIR"
pnpm exec tauri build "$@"

# Tauri signed the app and the .dmg around it already.
if [[ "$signing_identity" != "-" ]]; then
  exit 0
fi

app_bundle="$(ls -dt "$bundle_dir"/*.app 2>/dev/null | head -n 1 || true)"
if [[ -z "$app_bundle" ]]; then
  echo "No .app found under $bundle_dir; skipping ad-hoc signing." >&2
  exit 0
fi

sign_app "$app_bundle" "-" "$(entitlements_for "$@")"
warn_adhoc "$app_bundle"

cat >&2 <<EOF
The .dmg, if one was produced, still holds the app as it was before signing.
Install from $app_bundle instead.
EOF
