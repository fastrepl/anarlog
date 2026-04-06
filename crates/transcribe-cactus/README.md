```bash
cargo run --release -p transcribe-cactus --example live -- \
  --audio mock \
  --stream-chunk-sec 0.2 \
  --min-chunk-sec 2.0 \
  --model "<PATH>"
```

Path example: 
- `$HOME/Library/Application Support/com.hyprnote.dev/models/cactus/parakeet-tdt-0.6b-v3-int8-apple`
- `$HOME/Library/Application Support/com.hyprnote.dev/models/cactus/whisper-small-int8-apple`
