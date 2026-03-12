Direct Deepgram:

```bash
DEEPGRAM_API_KEY=... cargo run -p transcribe-cli --example deepgram -- --audio input
```

Direct Soniox:

```bash
SONIOX_API_KEY=... cargo run -p transcribe-cli --example soniox -- --audio input
```

Local Cactus:

```bash
cargo run -p transcribe-cli --example cactus -- --model /path/to/model.bin --audio input
```

Proxy testing:

```bash
DEEPGRAM_API_KEY=... cargo run -p transcribe-cli --example hyprnote -- --provider deepgram --audio input
```

```bash
DEEPGRAM_API_KEY=... SONIOX_API_KEY=... cargo run -p transcribe-cli --example hyprnote -- --provider hyprnote --audio input
```

Use `--audio output` instead of `--audio input` to transcribe speaker output.
