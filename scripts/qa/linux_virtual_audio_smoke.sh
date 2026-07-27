#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <candidate.deb>" >&2
  exit 2
fi

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Linux virtual-audio QA must run on Linux" >&2
  exit 1
fi

script_path=$(realpath "$0")
repo_root=$(cd "$(dirname "$script_path")/../.." && pwd)
deb_path=$(realpath "$1")
artifact_dir=$(realpath -m "${QA_ARTIFACT_DIR:-$repo_root/qa-artifacts/linux-audio}")

expected_version=${EXPECTED_VERSION:?EXPECTED_VERSION is required}
expected_debian_arch=${EXPECTED_DEBIAN_ARCH:?EXPECTED_DEBIAN_ARCH is required}

mkdir -p "$artifact_dir"
cat > "$artifact_dir/scope.txt" <<'EOF'
This QA run uses NO_AEC=1. It proves virtual microphone/system routing,
capture, separation, and persistence only. It does not validate acoustic echo
cancellation, physical devices, drivers, or real-room acoustics.
EOF

if [[ "${ANARLOG_AUDIO_QA_IN_DBUS:-0}" != "1" ]]; then
  for command in dbus-run-session dpkg-deb ffmpeg pnpm python3; do
    if ! command -v "$command" >/dev/null 2>&1; then
      echo "Missing required command: $command" >&2
      exit 1
    fi
  done

  package_version=$(dpkg-deb --field "$deb_path" Version)
  package_arch=$(dpkg-deb --field "$deb_path" Architecture)
  package_name=$(dpkg-deb --field "$deb_path" Package)

  if [[ "$package_version" != "$expected_version" ]]; then
    echo "Expected Debian version $expected_version, got $package_version" >&2
    exit 1
  fi
  if [[ "$package_arch" != "$expected_debian_arch" ]]; then
    echo "Expected Debian architecture $expected_debian_arch, got $package_arch" >&2
    exit 1
  fi

  sha256sum "$deb_path" | awk '{print $1 "  candidate.deb"}' \
    > "$artifact_dir/candidate.deb.sha256"
  {
    echo "package=$package_name"
    echo "version=$package_version"
    echo "architecture=$package_arch"
    echo "source=$deb_path"
  } > "$artifact_dir/debian-package.txt"

  sudo apt-get install -y "$deb_path" 2>&1 \
    | tee "$artifact_dir/debian-install.log"

  mapfile -t package_binaries < <(
    dpkg-query -L "$package_name" \
      | grep -E '^/usr/bin/anarlog(-staging)?$' \
      | sort -u
  )
  if [[ ${#package_binaries[@]} -ne 1 ]]; then
    printf "Expected one Anarlog executable, found: %s\n" \
      "${package_binaries[*]:-none}" >&2
    exit 1
  fi
  if [[ ! -x "${package_binaries[0]}" ]]; then
    echo "Installed Anarlog executable is not runnable" >&2
    exit 1
  fi

  qa_root=$(mktemp -d -t anarlog-linux-audio-qa.XXXXXXXX)
  install -d -m 700 "$qa_root/runtime"
  mkdir -p \
    "$qa_root/cache" \
    "$qa_root/config" \
    "$qa_root/data" \
    "$qa_root/state" \
    "$qa_root/vault"

  cat > "$qa_root/asound.conf" <<'EOF'
pcm.!default {
  type pulse
}

ctl.!default {
  type pulse
}
EOF

  export ALSA_CONFIG_PATH="$qa_root/asound.conf"
  export ANARLOG_AUDIO_QA_IN_DBUS=1
  export APP_BINARY_PATH="${package_binaries[0]}"
  export CHAR_VAULT_BASE="$qa_root/vault"
  export PULSE_SERVER="unix:$qa_root/runtime/pulse/native"
  export PIPEWIRE_RUNTIME_DIR="$qa_root/runtime"
  export QA_ARTIFACT_DIR="$artifact_dir"
  export QA_ROOT="$qa_root"
  export XDG_CACHE_HOME="$qa_root/cache"
  export XDG_CONFIG_HOME="$qa_root/config"
  export XDG_DATA_HOME="$qa_root/data"
  export XDG_RUNTIME_DIR="$qa_root/runtime"
  export XDG_STATE_HOME="$qa_root/state"

  exec dbus-run-session -- "$script_path" "$deb_path"
fi

for command in arecord pactl paplay pipewire pipewire-pulse pw-cli wireplumber; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing required audio command: $command" >&2
    exit 1
  fi
done

service_pids=()
cleanup() {
  for pid in "${service_pids[@]}"; do
    kill "$pid" >/dev/null 2>&1 || true
  done
  for pid in "${service_pids[@]}"; do
    wait "$pid" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT

pipewire > "$artifact_dir/pipewire.log" 2>&1 &
service_pids+=("$!")

wait_for() {
  local description=$1
  shift
  for _ in $(seq 1 100); do
    if "$@" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
  done
  echo "Timed out waiting for $description" >&2
  return 1
}

wait_for "PipeWire core" pw-cli info 0

wireplumber > "$artifact_dir/wireplumber.log" 2>&1 &
service_pids+=("$!")
pipewire-pulse > "$artifact_dir/pipewire-pulse.log" 2>&1 &
service_pids+=("$!")

wait_for "PipeWire Pulse server" pactl info

system_module=$(pactl load-module module-null-sink \
  sink_name=qa_system rate=48000 channels=1)
mic_module=$(pactl load-module module-null-sink \
  sink_name=qa_mic_bus rate=48000 channels=1)
pactl set-default-sink qa_system
pactl set-default-source qa_mic_bus.monitor

{
  echo "system_module=$system_module"
  echo "mic_module=$mic_module"
} > "$artifact_dir/virtual-audio-modules.txt"
pactl info > "$artifact_dir/pactl-info.txt"
pactl list short sinks > "$artifact_dir/pactl-sinks.txt"
pactl list short sources > "$artifact_dir/pactl-sources.txt"
pw-cli info 0 > "$artifact_dir/pipewire-core.txt"
arecord -L > "$artifact_dir/alsa-pcms.txt"

mic_signal="$QA_ROOT/mic-speech-with-523hz-pilot.wav"
system_signal="$QA_ROOT/system-997hz-tone.wav"
preflight_signal="$QA_ROOT/preflight-mic.wav"
fixture="$repo_root/crates/aec/data/inputs/theo_mic.wav"

ffmpeg -hide_banner -loglevel error -y \
  -i "$fixture" \
  -f lavfi -i "sine=frequency=523:sample_rate=48000:duration=10" \
  -filter_complex \
  "[0:a]atrim=duration=10,aresample=48000,volume=0.75[speech];[1:a]volume=0.02[pilot];[speech][pilot]amix=inputs=2:duration=shortest:normalize=0[out]" \
  -map "[out]" -ar 48000 -ac 1 -c:a pcm_s16le "$mic_signal"
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "sine=frequency=997:sample_rate=48000:duration=10" \
  -ar 48000 -ac 1 -c:a pcm_s16le "$system_signal"
ffmpeg -hide_banner -loglevel error -y \
  -i "$mic_signal" -t 3 -c:a pcm_s16le "$preflight_signal"

arecord \
  --device=default \
  --format=S16_LE \
  --rate=48000 \
  --channels=1 \
  --duration=4 \
  "$artifact_dir/alsa-mic-preflight.wav" \
  > "$artifact_dir/alsa-mic-preflight.log" 2>&1 &
preflight_pid=$!
sleep 0.25
paplay --device=qa_mic_bus "$preflight_signal"
wait "$preflight_pid"

ffmpeg -hide_banner -nostats \
  -i "$artifact_dir/alsa-mic-preflight.wav" \
  -af volumedetect -f null - \
  > /dev/null 2> "$artifact_dir/alsa-mic-preflight-volume.txt"
preflight_mean_db=$(
  awk '/mean_volume:/ {print $(NF - 1)}' \
    "$artifact_dir/alsa-mic-preflight-volume.txt" \
    | tail -n 1
)
if [[ -z "$preflight_mean_db" || "$preflight_mean_db" == "-inf" ]] \
  || ! awk -v value="$preflight_mean_db" \
    'BEGIN { exit !(value > -70.0) }'; then
  echo "ALSA-to-Pulse microphone preflight was silent" >&2
  exit 1
fi

export CI=1
export E2E_VIRTUAL_AUDIO=1
export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export LISTENER_DEBUG=1
export NO_AEC=1
export ONBOARDING=0
export QA_MIC_SIGNAL="$mic_signal"
export QA_MIC_SINK=qa_mic_bus
export QA_PHASES_PATH="$artifact_dir/phases.json"
export QA_SYSTEM_SIGNAL="$system_signal"
export QA_SYSTEM_SINK=qa_system
export RUST_LOG="info,sqlx=warn,hyper=warn"

set +e
pnpm -F e2e-blackbox e2e 2>&1 | tee "$artifact_dir/e2e.log"
e2e_status=${PIPESTATUS[0]}
set -e

mkdir -p "$artifact_dir/app-logs"
while IFS= read -r -d '' log_path; do
  relative_path=${log_path#"$QA_ROOT/"}
  cp "$log_path" "$artifact_dir/app-logs/${relative_path//\//__}"
done < <(
  find "$QA_ROOT/data" "$QA_ROOT/state" \
    -type f -name '*.log' -print0 2>/dev/null
)

for _ in $(seq 1 100); do
  mic_candidate=$(find "$CHAR_VAULT_BASE" -type f -name audio_mic.wav -print -quit)
  speaker_candidate=$(find "$CHAR_VAULT_BASE" -type f -name audio_spk.wav -print -quit)
  if [[ -n "$mic_candidate" && -n "$speaker_candidate" ]] \
    && ffprobe -v error "$mic_candidate" >/dev/null 2>&1 \
    && ffprobe -v error "$speaker_candidate" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

mapfile -t mic_tracks < <(
  find "$CHAR_VAULT_BASE" -type f -name audio_mic.wav -print
)
mapfile -t speaker_tracks < <(
  find "$CHAR_VAULT_BASE" -type f -name audio_spk.wav -print
)

verification_status=0
if [[ ${#mic_tracks[@]} -ne 1 || ${#speaker_tracks[@]} -ne 1 ]]; then
  echo "Expected one persisted mic track and one persisted speaker track" \
    | tee "$artifact_dir/track-discovery-error.txt"
  find "$CHAR_VAULT_BASE" -maxdepth 5 -type f -print \
    > "$artifact_dir/vault-files.txt"
  verification_status=1
else
  cp "${mic_tracks[0]}" "$artifact_dir/audio_mic.wav"
  cp "${speaker_tracks[0]}" "$artifact_dir/audio_spk.wav"

  set +e
  python3 "$repo_root/scripts/qa/verify_linux_audio_tracks.py" \
    --mic "$artifact_dir/audio_mic.wav" \
    --speaker "$artifact_dir/audio_spk.wav" \
    --phases "$artifact_dir/phases.json" \
    --output "$artifact_dir/analysis.json" \
    2>&1 | tee "$artifact_dir/verification.log"
  verification_status=${PIPESTATUS[0]}
  set -e
fi

grep -R -E \
  'capture_stream_failed|failed_to_send_audio_to_recorder|mic_queue_overflow|mic_stream_build_failed|mic_stream_error|mic_stream_start_failed|pipewire_capture_thread_failed|pipewire_stream_error|pulseaudio_capture_thread_failed|samples_dropped|source_stream_failed_stopping|speaker_queue_overflow' \
  "$artifact_dir/e2e.log" "$artifact_dir/app-logs" \
  > "$artifact_dir/capture-errors.txt" 2>/dev/null || true
grep -R -E \
  'pipewire_capture_initialized|pipewire_capture_unavailable|pulseaudio_capture_initialized' \
  "$artifact_dir/e2e.log" "$artifact_dir/app-logs" \
  > "$artifact_dir/capture-backend-events.txt" 2>/dev/null || true

capture_log_status=0
if [[ -s "$artifact_dir/capture-errors.txt" ]]; then
  echo "Capture error events were present in application logs" >&2
  capture_log_status=1
fi
if ! grep -q 'pipewire_capture_initialized' \
  "$artifact_dir/capture-backend-events.txt"; then
  echo "Application logs do not prove PipeWire capture initialized" >&2
  capture_log_status=1
fi
if grep -Eq \
  'pipewire_capture_unavailable|pulseaudio_capture_initialized' \
  "$artifact_dir/capture-backend-events.txt"; then
  echo "Application fell back from the required PipeWire capture backend" >&2
  capture_log_status=1
fi

if [[ $e2e_status -ne 0 ]]; then
  echo "Desktop E2E recording scenario failed with status $e2e_status" >&2
fi
if [[ $verification_status -ne 0 ]]; then
  echo "Persisted audio verification failed" >&2
fi

if [[ $e2e_status -ne 0 || $verification_status -ne 0 || $capture_log_status -ne 0 ]]; then
  exit 1
fi
