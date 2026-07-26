#!/bin/bash

if [ $# -eq 0 ]; then
  echo "Usage: $0 <youtube_url>"
  exit 1
fi

URL=$1
pipx run yt-dlp -f bestaudio --extract-audio --audio-format mp3 -o "./out.mp3" "$URL"
