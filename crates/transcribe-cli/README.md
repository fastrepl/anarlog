Direct Deepgram:

```bash
DEEPGRAM_API_KEY=... cargo run -p transcribe-cli --example deepgram -- --audio input
```

```bash
DEEPGRAM_API_KEY=... cargo run -p transcribe-cli --example deepgram -- --audio aec-dual
```

Direct Soniox:

```bash
SONIOX_API_KEY=... cargo run -p transcribe-cli --example soniox -- --audio input
```

```bash
SONIOX_API_KEY=... cargo run -p transcribe-cli --example soniox -- --audio raw-dual
```

Local Cactus:

```bash
cargo run -p transcribe-cli --example cactus -- --model /path/to/model.bin --audio input
```

```bash
cargo run -p transcribe-cli --example cactus -- --model /path/to/model.bin --audio aec-dual
```

Proxy testing:

```bash
DEEPGRAM_API_KEY=... cargo run -p transcribe-cli --example hyprnote -- --provider deepgram --audio input
```

```bash
DEEPGRAM_API_KEY=... SONIOX_API_KEY=... cargo run -p transcribe-cli --example hyprnote -- --provider hyprnote --audio input
```

Use `--audio output` to transcribe speaker output, `--audio raw-dual` for raw mic + speaker, and `--audio aec-dual` for AEC mic + speaker.
