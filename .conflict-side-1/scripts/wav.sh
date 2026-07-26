#!/bin/bash

if [ $# -eq 0 ]; then
  echo "Error: Please provide a path to a WAV file"
  echo "Usage: $0 <path_to_wav_file>"
  exit 1
fi

input_path="$1"

if [[ ! "$input_path" = /* ]]; then
  input_path="$(pwd)/$input_path"
fi

ffmpeg -i "$input_path" -f lavfi -i color=c=white:s=640x120 -filter_complex "[0:a]showwavespic=s=640x120:colors=black[fg];[1:v][fg]overlay=format=auto" -frames:v 1 -f image2pipe -vcodec png - | open -a Preview -f
