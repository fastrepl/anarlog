#!/usr/bin/env bash
# Encodes render.mjs frames into a faststart MP4 and lays the narration under it untouched: one
# take, delayed to NARRATION_AT, padded to the film's length. No trimming, re-timing or loudness
# changes. With MUSIC set to the film's music from music.py (already held low under the voice), the
# music is mixed under the take; the take itself is added sample for sample.
#   [MUSIC=music.wav] [NARRATION_AT=2.2] mux.sh <framesDir> <out.mp4> [narration.mp3 [narrationAt]] [fps]
set -euo pipefail
usage="usage: [MUSIC=music.wav] [NARRATION_AT=s] mux.sh <framesDir> <out.mp4> [narration.mp3 [narrationAt]] [fps]"
frames="${1:?$usage}"
out="${2:?$usage}"
take="${3:-}"
at="${4:-${NARRATION_AT:-0}}"
fps="${5:-30}"
music="${MUSIC:-}"
[ -e "$frames/f00000.png" ] || { echo "no f00000.png in $frames: run render.mjs frames first" >&2; exit 1; }
[ -z "$take" ] || [ -f "$take" ] || { echo "narration not found: $take" >&2; exit 1; }
[ -z "$music" ] || [ -f "$music" ] || { echo "music not found: $music" >&2; exit 1; }
video=(-framerate "$fps" -i "$frames/f%05d.png")
venc=(-c:v libx264 -pix_fmt yuv420p -crf 18 -preset medium -movflags +faststart)
aenc=(-c:a aac -b:a 192k -ar 48000 -ac 2)
ms=$(python3 -c "print(int(round(float('$at') * 1000)))")
voice="[1:a]adelay=$ms:all=1,apad"
if [ -z "$take" ] && [ -z "$music" ]; then
  ffmpeg -y -loglevel error "${video[@]}" "${venc[@]}" "$out"
elif [ -z "$music" ]; then
  ffmpeg -y -loglevel error "${video[@]}" -i "$take" \
    -filter_complex "$voice[a]" -map 0:v -map "[a]" -shortest "${venc[@]}" "${aenc[@]}" "$out"
elif [ -z "$take" ]; then
  ffmpeg -y -loglevel error "${video[@]}" -i "$music" -map 0:v -map 1:a -shortest "${venc[@]}" "${aenc[@]}" "$out"
else
  ffmpeg -y -loglevel error "${video[@]}" -i "$take" -i "$music" \
    -filter_complex "$voice,aformat=channel_layouts=stereo[v];[v][2:a]amix=inputs=2:normalize=0:duration=first[a]" \
    -map 0:v -map "[a]" -shortest "${venc[@]}" "${aenc[@]}" "$out"
fi
echo "$out: $(ffprobe -v error -show_entries format=duration -of csv=p=0 "$out")s"
