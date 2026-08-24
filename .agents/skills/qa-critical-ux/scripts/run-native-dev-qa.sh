#!/usr/bin/env bash

set -euo pipefail
umask 077

qa_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
qa_target_dir="${ANARLOG_QA_TARGET_DIR:-$HOME/Library/Caches/anarlog/native-dev-qa-target-v2}"
qa_target_parent="$(dirname "$qa_target_dir")"
qa_bundle_dir="$qa_target_dir/debug/bundle/macos/Anarlog Dev.app"
qa_bundle_executable="$qa_target_dir/debug/bundle/macos/Anarlog Dev.app/Contents/MacOS/anarlog-dev"
qa_manifest="$qa_target_dir/.anarlog-native-dev-qa-manifest"
qa_cache_marker="$qa_target_dir/.anarlog-native-dev-qa-cache-v2"
qa_lock_dir="$qa_target_dir/.anarlog-native-dev-qa-lock"
qa_frontend_dist="$qa_repo_root/apps/desktop/dist"
qa_config_validator="$qa_repo_root/.agents/skills/qa-critical-ux/scripts/validate-native-dev-qa-config.mjs"
qa_gitbutler_candidate_resolver="$qa_repo_root/.agents/skills/qa-critical-ux/scripts/resolve-gitbutler-candidate.mjs"
qa_launcher="$qa_repo_root/.agents/skills/qa-critical-ux/scripts/launch-native-dev-qa.sh"
qa_release_workflow="$qa_repo_root/.github/workflows/desktop_cd.yaml"
qa_expected_supabase_url="https://ijoptyyjrfqwaqhyxkxj.supabase.co"
qa_xcode_developer_dir="/Applications/Xcode.app/Contents/Developer"
qa_git_source_pathspecs=(
  ".agents/skills/qa-critical-ux/scripts"
  ".cargo"
  "apps/desktop"
  "crates"
  "legacy"
  "packages"
  "plugins"
  "scripts"
  ".github/workflows/desktop_cd.yaml"
  "Cargo.lock"
  "Cargo.toml"
  "package.json"
  "pnpm-lock.yaml"
  "pnpm-workspace.yaml"
  "rust-toolchain"
  "rust-toolchain.toml"
  ":(exclude,glob)**/dist/**"
  ":(exclude,glob)**/node_modules/**"
  ":(exclude,glob)**/.build/**"
  ":(exclude,glob)**/target/**"
  ":(exclude,glob)**/.env"
  ":(exclude,glob)**/.env.*"
)

fail() {
  echo "$1" >&2
  exit 1
}

sha256_file() {
  shasum -a 256 "$1" | awk '{print $1}'
}

validated_commit_sha() {
  local qa_requested_sha="$1"
  local qa_normalized_sha
  local qa_resolved_sha

  [[ "$qa_requested_sha" =~ ^[0-9a-fA-F]{40}$ ]] ||
    fail "ANARLOG_QA_GIT_SHA must be a full 40-character commit SHA."
  qa_normalized_sha="$(
    printf '%s' "$qa_requested_sha" |
      tr '[:upper:]' '[:lower:]'
  )"
  qa_resolved_sha="$(
    "$qa_git_executable" -C "$qa_repo_root" \
      rev-parse --verify "$qa_normalized_sha^{commit}" 2>/dev/null
  )" || fail "ANARLOG_QA_GIT_SHA does not resolve to a local Git commit."
  [[ "$qa_resolved_sha" == "$qa_normalized_sha" ]] ||
    fail "ANARLOG_QA_GIT_SHA must identify the commit object directly."
  printf '%s' "$qa_resolved_sha"
}

candidate_git_sha() {
  local qa_current_branch
  local qa_derived_sha
  local qa_but_executable

  if [[ -n "${ANARLOG_QA_GIT_SHA:-}" ]]; then
    qa_derived_sha="$(
      "$qa_node_executable" \
        "$qa_gitbutler_candidate_resolver" \
        "$ANARLOG_QA_GIT_SHA"
    )" || fail "ANARLOG_QA_GIT_SHA is not a valid candidate commit."
  else
    qa_current_branch="$(
      "$qa_git_executable" -C "$qa_repo_root" \
        symbolic-ref --quiet --short HEAD 2>/dev/null || true
    )"
    if [[ "$qa_current_branch" == "gitbutler/workspace" ]]; then
      qa_but_executable="$(command -v but)" ||
        fail "GitButler is required to derive the QA candidate. Set ANARLOG_QA_GIT_SHA explicitly."
      qa_derived_sha="$(
        (
          cd "$qa_repo_root"
          "$qa_but_executable" status --format json
        ) |
          "$qa_node_executable" "$qa_gitbutler_candidate_resolver"
      )" || fail "Could not derive an unambiguous GitButler candidate commit."
    else
      qa_derived_sha="$(
        "$qa_git_executable" -C "$qa_repo_root" rev-parse --verify HEAD
      )" || fail "Could not derive the current Git commit."
    fi
  fi

  validated_commit_sha "$qa_derived_sha"
}

candidate_source_dirty_state() {
  local qa_candidate_sha="$1"
  local qa_candidate_index="$qa_lock_dir/candidate-index"
  local qa_diff_status=0
  local qa_refresh_status=0
  local qa_untracked_files

  rm -f -- "$qa_candidate_index"
  GIT_INDEX_FILE="$qa_candidate_index" \
    "$qa_git_executable" -C "$qa_repo_root" read-tree "$qa_candidate_sha" || {
    rm -f -- "$qa_candidate_index"
    fail "Could not load the candidate commit for QA build-input comparison."
  }
  GIT_INDEX_FILE="$qa_candidate_index" \
    "$qa_git_executable" -C "$qa_repo_root" update-index -q --refresh ||
    qa_refresh_status=$?
  case "$qa_refresh_status" in
    0 | 1) ;;
    *)
      rm -f -- "$qa_candidate_index"
      fail "Could not refresh the candidate index for QA build-input comparison."
      ;;
  esac
  GIT_INDEX_FILE="$qa_candidate_index" \
    "$qa_git_executable" -C "$qa_repo_root" diff-files --quiet \
    -- "${qa_git_source_pathspecs[@]}" ||
    qa_diff_status=$?
  case "$qa_diff_status" in
    0) ;;
    1)
      rm -f -- "$qa_candidate_index"
      printf 'true'
      return
      ;;
    *)
      rm -f -- "$qa_candidate_index"
      fail "Could not compare QA build inputs with the candidate commit."
      ;;
  esac

  qa_untracked_files="$(
    GIT_INDEX_FILE="$qa_candidate_index" \
      "$qa_git_executable" -C "$qa_repo_root" ls-files \
      --others \
      --exclude-standard \
      -- "${qa_git_source_pathspecs[@]}"
  )"
  rm -f -- "$qa_candidate_index"
  if [[ -n "$qa_untracked_files" ]]; then
    printf 'true'
  else
    printf 'false'
  fi
}

validate_builtin_audio_devices() {
  local qa_audio_report

  [[ -x /usr/sbin/system_profiler ]] ||
    fail "system_profiler is required to validate the macOS QA audio devices."
  qa_audio_report="$(/usr/sbin/system_profiler SPAudioDataType -json)"
  printf '%s' "$qa_audio_report" |
    "$qa_node_executable" -e '
      let source = "";
      process.stdin.on("data", (chunk) => (source += chunk));
      process.stdin.on("end", () => {
        let report;
        try {
          report = JSON.parse(source);
        } catch {
          console.error("Could not parse the macOS audio-device report.");
          process.exit(1);
        }
        const devices = (report.SPAudioDataType ?? []).flatMap(
          (group) => group._items ?? [],
        );
        const input = devices.find(
          (device) =>
            device.coreaudio_default_audio_input_device === "spaudio_yes",
        );
        const output = devices.find(
          (device) =>
            device.coreaudio_default_audio_output_device === "spaudio_yes",
        );
        const isBuiltIn = (device) =>
          device?.coreaudio_device_transport ===
          "coreaudio_device_type_builtin";
        const name = (device) => device?._name ?? "none";
        if (!isBuiltIn(input) || !isBuiltIn(output)) {
          console.error(
            `Native QA requires built-in Mac audio defaults. Current input: ${name(input)}; output: ${name(output)}.`,
          );
          process.exit(1);
        }
        console.log(
          `Native QA audio defaults: input=${name(input)}; output=${name(output)}`,
        );
      });
    ' ||
    fail "Select the built-in MacBook microphone and speakers, then rerun native QA."
}

workflow_public_endpoint() {
  local qa_endpoint_key="$1"
  local qa_endpoint_values
  local qa_endpoint_count

  qa_endpoint_values="$(
    awk -v key="$qa_endpoint_key" '
      $1 == key ":" {
        value = $2
        sub(/^["\047]/, "", value)
        sub(/["\047]$/, "", value)
        print value
      }
    ' "$qa_release_workflow" |
      LC_ALL=C sort -u
  )"
  qa_endpoint_count="$(
    printf '%s\n' "$qa_endpoint_values" |
      sed '/^$/d' |
      wc -l |
      tr -d ' '
  )"
  [[ "$qa_endpoint_count" == "1" ]] ||
    fail "Expected one $qa_endpoint_key value in desktop_cd.yaml."
  case "$qa_endpoint_values" in
    https://*) ;;
    *) fail "$qa_endpoint_key in desktop_cd.yaml must be an HTTPS URL." ;;
  esac
  printf '%s' "$qa_endpoint_values"
}

public_config_fingerprint() {
  {
    printf 'VITE_APP_URL=%s\n' "$qa_production_app_url"
    printf 'VITE_API_URL=%s\n' "$qa_production_api_url"
    printf 'VITE_SUPABASE_URL=%s\n' "$qa_supabase_url"
    printf 'VITE_SUPABASE_ANON_KEY=%s\n' "$qa_supabase_anon_key"
  } |
    shasum -a 256 |
    awk '{print $1}'
}

bundle_config_binding() {
  {
    printf 'bundle_sha256=%s\n' "$1"
    printf 'frontend_config_sha256=%s\n' "$2"
  } |
    shasum -a 256 |
    awk '{print $1}'
}

compiled_frontend_config_fingerprint() {
  env -i \
    PATH="$PATH" \
    HOME="$HOME" \
    ANARLOG_QA_EXPECTED_VITE_APP_URL="$qa_production_app_url" \
    ANARLOG_QA_EXPECTED_VITE_API_URL="$qa_production_api_url" \
    ANARLOG_QA_EXPECTED_VITE_SUPABASE_URL="$qa_supabase_url" \
    ANARLOG_QA_EXPECTED_VITE_SUPABASE_ANON_KEY="$qa_supabase_anon_key" \
    "$qa_node_executable" "$qa_config_validator" "$qa_frontend_dist"
}

bundle_fingerprint() {
  (
    cd "$qa_bundle_dir"
    find . \( -type f -o -type l \) |
      LC_ALL=C sort |
      COPYFILE_DISABLE=1 tar -cf - --no-recursion -T - 2>/dev/null
  ) |
    shasum -a 256 |
    awk '{print $1}'
}

source_fingerprint() {
  (
    cd "$qa_repo_root"
    {
      for qa_source_root in \
        .agents/skills/qa-critical-ux/scripts \
        .cargo \
        apps/desktop \
        crates \
        legacy \
        packages \
        plugins \
        scripts; do
        if [[ -e "$qa_source_root" ]]; then
          find "$qa_source_root" \
            \( \
              -path '*/dist' -o \
              -path '*/node_modules' -o \
              -path '*/.build' -o \
              -path '*/target' -o \
              -name '.env' -o \
              -name '.env.*' \
            \) -prune -o \
            \( -type f -o -type l \) -print
        fi
      done
      for qa_source_file in \
        .github/workflows/desktop_cd.yaml \
        Cargo.lock \
        Cargo.toml \
        package.json \
        pnpm-lock.yaml \
        pnpm-workspace.yaml \
        rust-toolchain \
        rust-toolchain.toml; do
        if [[ -e "$qa_source_file" ]]; then
          printf '%s\n' "$qa_source_file"
        fi
      done
    } |
      LC_ALL=C sort -u |
      COPYFILE_DISABLE=1 tar -cf - \
        --format=mtree \
        --options='!all,type,mode,link,sha256' \
        --no-recursion \
        -T - \
        2>/dev/null
  ) |
    shasum -a 256 |
    awk '{print $1}'
}

load_production_config() {
  qa_site_html="$(
    curl -fsSL \
      --retry 3 \
      --connect-timeout 10 \
      --max-time 30 \
      "$qa_production_app_url/"
  )"
  qa_env_asset_paths="$(
    {
      printf '%s' "$qa_site_html" |
        rg --text -o '/assets/env-[^"]+\.js' || true
    } |
      LC_ALL=C sort -u
  )"
  qa_env_asset_count="$(
    printf '%s\n' "$qa_env_asset_paths" |
      sed '/^$/d' |
      wc -l |
      tr -d ' '
  )"
  [[ "$qa_env_asset_count" == "1" ]] ||
    fail "Expected one deployed public environment asset."
  qa_env_asset_path="$qa_env_asset_paths"

  qa_env_asset="$(
    curl -fsSL \
      --retry 3 \
      --connect-timeout 10 \
      --max-time 30 \
      "$qa_production_app_url$qa_env_asset_path"
  )"
  qa_supabase_urls="$(
    {
      printf '%s' "$qa_env_asset" |
        rg --text -o 'https://[a-z0-9-]+\.supabase\.co' || true
    } |
      LC_ALL=C sort -u
  )"
  qa_supabase_keys="$(
    {
      printf '%s' "$qa_env_asset" |
        rg --text -o 'eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+' || true
    } |
      LC_ALL=C sort -u
  )"
  qa_supabase_url_count="$(
    printf '%s\n' "$qa_supabase_urls" |
      sed '/^$/d' |
      wc -l |
      tr -d ' '
  )"
  qa_supabase_key_count="$(
    printf '%s\n' "$qa_supabase_keys" |
      sed '/^$/d' |
      wc -l |
      tr -d ' '
  )"
  [[ "$qa_supabase_url_count" == "1" ]] ||
    fail "Expected one deployed public Supabase URL."
  [[ "$qa_supabase_key_count" == "1" ]] ||
    fail "Expected one deployed public Supabase key."
  qa_supabase_url="$qa_supabase_urls"
  qa_supabase_anon_key="$qa_supabase_keys"

  [[ "$qa_supabase_url" == "$qa_expected_supabase_url" ]] ||
    fail "Unexpected production Supabase project."
  printf '%s' "$qa_supabase_anon_key" |
    "$qa_node_executable" -e '
      let token = "";
      process.stdin.on("data", (chunk) => (token += chunk));
      process.stdin.on("end", () => {
        try {
          let payload = token.split(".")[1] ?? "";
          payload = payload.replace(/-/g, "+").replace(/_/g, "/");
          payload += "=".repeat((4 - (payload.length % 4)) % 4);
          const claims = JSON.parse(Buffer.from(payload, "base64").toString());
          if (
            claims.role !== "anon" ||
            claims.ref !== "ijoptyyjrfqwaqhyxkxj"
          ) {
            process.exit(1);
          }
        } catch {
          process.exit(1);
        }
      });
    ' ||
    fail "The deployed Supabase key is not the expected public anon key."

  qa_env_asset_sha256="$(
    printf '%s' "$qa_env_asset" |
      shasum -a 256 |
      awk '{print $1}'
  )"
  qa_anon_key_sha256="$(
    printf '%s' "$qa_supabase_anon_key" |
      shasum -a 256 |
      awk '{print $1}'
  )"
}

manifest_value() {
  awk -F= -v key="$1" '$1 == key { print substr($0, length(key) + 2) }' "$qa_manifest"
}

validate_manifest() {
  local qa_current_bundle_sha256
  local qa_expected_frontend_config_sha256
  local qa_expected_bundle_config_binding
  local qa_current_git_head_sha
  local qa_current_git_dirty

  [[ -f "$qa_manifest" ]] ||
    fail "Dev QA manifest not found. Build the bundle without --launch-only first."
  [[ -x "$qa_bundle_executable" ]] ||
    fail "Dev QA bundle not found: $qa_bundle_executable"
  [[ "$(manifest_value app_url)" == "$qa_production_app_url" ]] ||
    fail "Dev QA manifest does not target the production web app."
  [[ "$(manifest_value api_url)" == "$qa_production_api_url" ]] ||
    fail "Dev QA manifest does not target the production API."
  [[ "$(manifest_value supabase_url)" == "$qa_supabase_url" ]] ||
    fail "Dev QA manifest does not target the production Supabase project."
  [[ "$(manifest_value anon_key_sha256)" == "$qa_anon_key_sha256" ]] ||
    fail "The deployed public auth key changed. Rebuild the Dev QA bundle."
  qa_expected_frontend_config_sha256="$(public_config_fingerprint)"
  [[ "$(manifest_value frontend_config_sha256)" == "$qa_expected_frontend_config_sha256" ]] ||
    fail "Dev QA manifest does not attest the current compiled public configuration."
  qa_current_bundle_sha256="$(bundle_fingerprint)"
  [[ "$(manifest_value bundle_sha256)" == "$qa_current_bundle_sha256" ]] ||
    fail "Dev QA app bundle changed after its successful build."
  qa_expected_bundle_config_binding="$(
    bundle_config_binding \
      "$qa_current_bundle_sha256" \
      "$qa_expected_frontend_config_sha256"
  )"
  [[ "$(manifest_value bundle_config_sha256)" == "$qa_expected_bundle_config_binding" ]] ||
    fail "Dev QA bundle is not bound to its validated public configuration."
  [[ "$(manifest_value source_fingerprint)" == "$(source_fingerprint)" ]] ||
    fail "Dev QA bundle is stale for the current source tree. Rebuild it first."
  qa_current_git_head_sha="$(candidate_git_sha)"
  qa_current_git_dirty="$(
    candidate_source_dirty_state "$qa_current_git_head_sha"
  )"
  [[ "$(manifest_value git_head_sha)" == "$qa_current_git_head_sha" ]] ||
    fail "Dev QA manifest was built for a different candidate commit. Rebuild it first."
  [[ "$(manifest_value git_dirty)" == "$qa_current_git_dirty" ]] ||
    fail "Dev QA candidate build-input state changed after the build. Rebuild it first."
}

[[ -f "$qa_release_workflow" ]] ||
  fail "Desktop release workflow not found: $qa_release_workflow"
[[ -f "$qa_config_validator" ]] ||
  fail "Compiled frontend validator not found: $qa_config_validator"
[[ -f "$qa_gitbutler_candidate_resolver" ]] ||
  fail "GitButler candidate resolver not found: $qa_gitbutler_candidate_resolver"
[[ -x "$qa_launcher" ]] ||
  fail "Native Dev QA launcher not found: $qa_launcher"
qa_node_executable="$(command -v node)" ||
  fail "Node.js is required for native Dev QA configuration validation."
qa_git_executable="$(command -v git)" ||
  fail "Git is required for native Dev QA provenance."
candidate_git_sha >/dev/null
qa_production_app_url="$(workflow_public_endpoint VITE_APP_URL)"
qa_production_api_url="$(workflow_public_endpoint VITE_API_URL)"
validate_builtin_audio_devices

case "$qa_target_dir" in
  /*) ;;
  *) fail "ANARLOG_QA_TARGET_DIR must be an absolute path." ;;
esac
mkdir -p "$qa_target_parent"
qa_target_parent_real="$(cd "$qa_target_parent" && pwd -P)"
qa_target_real="$qa_target_parent_real/$(basename "$qa_target_dir")"
qa_home_real="$(cd "$HOME" && pwd -P)"

case "$qa_target_real" in
  "$qa_repo_root" | "$qa_repo_root"/*)
    fail "ANARLOG_QA_TARGET_DIR must be outside the repository."
    ;;
esac
case "$qa_target_real" in
  / | /private | /private/tmp | /tmp | "$qa_home_real" | "$qa_home_real/Library" | "$qa_home_real/Library/Caches")
    fail "ANARLOG_QA_TARGET_DIR must be a dedicated cache directory."
    ;;
esac
[[ ! -L "$qa_target_dir" ]] || fail "Refusing to use a symlinked QA target."

if [[ -e "$qa_target_dir" ]]; then
  [[ -d "$qa_target_dir" ]] || fail "The QA target exists but is not a directory."
  [[ "$(stat -f '%u' "$qa_target_dir")" == "$(id -u)" ]] ||
    fail "The QA target is not owned by the current user."
  [[ -f "$qa_cache_marker" ]] ||
    fail "Refusing to use an existing directory not created by this helper: $qa_target_dir"
else
  mkdir -m 700 "$qa_target_dir"
  printf 'anarlog-native-dev-qa-cache-v2\n' >"$qa_cache_marker"
fi
chmod 700 "$qa_target_dir"

if pgrep -x anarlog-dev >/dev/null 2>&1; then
  echo "Anarlog Dev is already running. Quit it before building or launching QA:" >&2
  pgrep -x anarlog-dev | while IFS= read -r qa_pid; do
    ps -p "$qa_pid" -o pid=,command= >&2
  done
  exit 1
fi

cleanup() {
  if [[ -f "$qa_lock_dir/pid" ]] &&
    [[ "$(cat "$qa_lock_dir/pid")" == "$$" ]]; then
    rm -f -- "$qa_lock_dir/candidate-index"
    rm -f -- "$qa_lock_dir/pid"
    rmdir "$qa_lock_dir" 2>/dev/null || true
  fi
}
if ! mkdir "$qa_lock_dir" 2>/dev/null; then
  [[ -f "$qa_lock_dir/pid" ]] ||
    fail "QA cache lock exists without owner metadata: $qa_lock_dir"
  qa_lock_pid="$(cat "$qa_lock_dir/pid")"
  case "$qa_lock_pid" in
    '' | *[!0-9]*)
      fail "QA cache lock has invalid owner metadata: $qa_lock_dir"
      ;;
  esac
  if kill -0 "$qa_lock_pid" 2>/dev/null; then
    fail "Another native Dev QA command is using this target (PID $qa_lock_pid)."
  fi
  rm -f -- "$qa_lock_dir/pid"
  rmdir "$qa_lock_dir" ||
    fail "Could not reclaim the stale QA cache lock: $qa_lock_dir"
  mkdir "$qa_lock_dir" ||
    fail "Could not acquire the QA cache lock: $qa_lock_dir"
fi
printf '%s\n' "$$" >"$qa_lock_dir/pid"
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

load_production_config

if [[ "${1:-}" == "--launch-only" ]]; then
  validate_manifest
elif [[ $# -eq 0 ]]; then
  [[ -d "$qa_xcode_developer_dir" ]] ||
    fail "Full Xcode is required to build the native Dev QA bundle."
  DEVELOPER_DIR="$qa_xcode_developer_dir" xcrun --find metal >/dev/null ||
    fail "The selected Xcode installation does not provide the Metal compiler."
  qa_pnpm_executable="$(command -v pnpm)" ||
    fail "pnpm is required to build the native Dev QA bundle."
  qa_build_user="$(id -un)"
  qa_build_env=(
    "PATH=$PATH"
    "HOME=$HOME"
    "USER=$qa_build_user"
    "LOGNAME=$qa_build_user"
    "TMPDIR=${TMPDIR:-/tmp}"
    "LANG=${LANG:-C}"
    "CI=false"
    "CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}"
    "RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}"
    "CARGO_TARGET_DIR=$qa_target_dir"
    "DEVELOPER_DIR=$qa_xcode_developer_dir"
    "VITE_APP_URL=$qa_production_app_url"
    "VITE_API_URL=$qa_production_api_url"
    "VITE_SUPABASE_URL=$qa_supabase_url"
    "VITE_SUPABASE_ANON_KEY=$qa_supabase_anon_key"
  )
  qa_git_head_sha_before="$(candidate_git_sha)"
  qa_git_dirty_before="$(
    candidate_source_dirty_state "$qa_git_head_sha_before"
  )"
  qa_source_fingerprint_before="$(source_fingerprint)"
  rm -f -- "$qa_manifest"
  (
    cd "$qa_repo_root"
    env -i "${qa_build_env[@]}" \
      "$qa_pnpm_executable" -F ui build
  )
  (
    cd "$qa_repo_root/apps/desktop"
    env -i "${qa_build_env[@]}" \
      "$qa_pnpm_executable" exec tauri build \
        --debug \
        --bundles app \
        --features dev \
        --config '{"bundle":{"createUpdaterArtifacts":false}}'
  )
  [[ -x "$qa_bundle_executable" ]] ||
    fail "The native Dev build completed without a launchable app bundle."
  qa_source_fingerprint_after="$(source_fingerprint)"
  [[ "$qa_source_fingerprint_before" == "$qa_source_fingerprint_after" ]] ||
    fail "Build inputs changed while the Dev QA bundle was being built. Rebuild it."
  qa_git_head_sha_after="$(candidate_git_sha)"
  qa_git_dirty_after="$(
    candidate_source_dirty_state "$qa_git_head_sha_after"
  )"
  [[ "$qa_git_head_sha_before" == "$qa_git_head_sha_after" ]] ||
    fail "The candidate Git commit changed while the Dev QA bundle was being built. Rebuild it."
  [[ "$qa_git_dirty_before" == "$qa_git_dirty_after" ]] ||
    fail "The candidate build-input state changed while the Dev QA bundle was being built. Rebuild it."
  qa_compiled_frontend_config_sha256="$(
    compiled_frontend_config_fingerprint
  )"
  qa_expected_frontend_config_sha256="$(public_config_fingerprint)"
  [[ "$qa_compiled_frontend_config_sha256" == "$qa_expected_frontend_config_sha256" ]] ||
    fail "Compiled frontend public configuration fingerprint is inconsistent."
  qa_bundle_sha256="$(bundle_fingerprint)"
  qa_bundle_config_sha256="$(
    bundle_config_binding \
      "$qa_bundle_sha256" \
      "$qa_compiled_frontend_config_sha256"
  )"

  qa_manifest_tmp="$qa_manifest.tmp.$$"
  {
    printf 'built_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf 'app_url=%s\n' "$qa_production_app_url"
    printf 'api_url=%s\n' "$qa_production_api_url"
    printf 'supabase_url=%s\n' "$qa_supabase_url"
    printf 'env_asset_url=%s%s\n' "$qa_production_app_url" "$qa_env_asset_path"
    printf 'env_asset_sha256=%s\n' "$qa_env_asset_sha256"
    printf 'anon_key_sha256=%s\n' "$qa_anon_key_sha256"
    printf 'git_head_sha=%s\n' "$qa_git_head_sha_before"
    printf 'git_dirty=%s\n' "$qa_git_dirty_before"
    printf 'source_fingerprint=%s\n' "$qa_source_fingerprint_before"
    printf 'executable_sha256=%s\n' "$(sha256_file "$qa_bundle_executable")"
    printf 'frontend_config_sha256=%s\n' "$qa_compiled_frontend_config_sha256"
    printf 'bundle_sha256=%s\n' "$qa_bundle_sha256"
    printf 'bundle_config_sha256=%s\n' "$qa_bundle_config_sha256"
  } >"$qa_manifest_tmp"
  mv "$qa_manifest_tmp" "$qa_manifest"
  load_production_config
  validate_manifest
else
  echo "Usage: $0 [--launch-only]" >&2
  exit 2
fi

echo "Native Dev QA manifest: $qa_manifest"
echo "Native Dev QA candidate: $(manifest_value git_head_sha) (dirty=$(manifest_value git_dirty))"
if [[ "$(manifest_value git_dirty)" == "true" ]]; then
  echo "This bundle is valid for exploratory QA but not exact-SHA release evidence." >&2
fi

cleanup
trap - EXIT INT TERM

cd "$qa_repo_root"
exec "$qa_launcher" "$qa_bundle_dir"
