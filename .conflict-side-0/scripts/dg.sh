#!/bin/bash

stream_url="https://playerservices.streamtheworld.com/api/livestream-redirect/CSPANRADIOAAC.aac"
stt_endpoint="wss://api.deepgram.com/v1/listen?provider=deepgram&model=nova-3&channels=1&sample_rate=16000&encoding=linear16&diarize=true&punctuate=true&smart_format=true&numerals=true&filler_words=false&mip_opt_out=true&interim_results=true&multichannel=false&vad_events=false&redemption_time_ms=400&language=en-CA&keyterm=type&keyterm=doc&keyterm=content&keyterm=paragraph"

ffmpeg -loglevel error -i "$stream_url" -f s16le -ar 16000 -ac 1 - | \
  websocat -v -H "Authorization: Token $DEEPGRAM_API_KEY" \
    -b --base64-text "$stt_endpoint" | \
  {
    while read -r msg; do
      if [[ -n "$msg" ]]; then
        json=$(echo "$msg" | base64 -d)
        is_final=$(echo "$json" | jq -r '.is_final // empty')
        transcript=$(echo "$json" | jq -r '.channel?.alternatives?[0]?.transcript? // empty')
        if [[ -n "$transcript" ]]; then
          prefix="[Interim]"
          if [[ "$is_final" == "true" ]]; then
            prefix="[ FINAL ]"
          fi
          echo "$prefix $transcript"
        fi
      fi
    done
  }
