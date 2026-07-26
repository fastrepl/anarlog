#!/bin/bash

# Check if input file is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <input_audio_file>"
    exit 1
fi

INPUT_FILE="$1"
DIR=$(dirname "$INPUT_FILE")
BASENAME=$(basename "$INPUT_FILE" .wav)

# Array of common sample rates for testing
SAMPLE_RATES=(8000 16000 22050 32000 44100 48000)

# Get duration of input file in seconds
DURATION=$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$INPUT_FILE")

# Calculate part duration
NUM_PARTS=${#SAMPLE_RATES[@]}
PART_DURATION=$(echo "$DURATION / $NUM_PARTS" | bc -l)

# Generate parts with different sample rates
for i in "${!SAMPLE_RATES[@]}"; do
    RATE=${SAMPLE_RATES[$i]}
    PART_NUM=$((i + 1))
    START=$(echo "$i * $PART_DURATION" | bc -l)
    OUTPUT_FILE="${DIR}/${BASENAME}_part${PART_NUM}_${RATE}hz.wav"
    
    echo "Creating part ${PART_NUM}: ${START}s-$(echo "$START + $PART_DURATION" | bc -l)s at ${RATE}Hz"
    ffmpeg -i "$INPUT_FILE" -ss ${START} -t ${PART_DURATION} -ar ${RATE} "$OUTPUT_FILE" -y -loglevel error
done

echo "Done! Created ${NUM_PARTS} parts with different sample rates in ${DIR}/"