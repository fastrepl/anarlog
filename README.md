> **Note:** The team is now building **[char](https://char.com)**. The **anarlog** community application remains open-source, MIT-licensed, and maintained as the local-first meeting notetaker in this repo. Source-visible enterprise components are commercially licensed.

![anarlog](https://repository-images.githubusercontent.com/900550981/a4267a9f-414b-4c36-965c-419313ce2417)

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/fastrepl/anarlog)

# anarlog

An open-source AI meeting notetaker that is local-first, privacy-first, and yours to fork.

Granola, rearranged.

## How to use it

Download the latest release for your platform:

→ [github.com/fastrepl/anarlog/releases/latest](https://github.com/fastrepl/anarlog/releases/latest)

Open it and join a meeting. anarlog records, transcribes locally, and stores its canonical meeting data in a local SQLite database. Export Markdown when it fits your workflow. Bring your own LLM: OpenAI, Anthropic, Gemini, OpenRouter, Ollama, LM Studio, Unsloth, or anything OpenAI-compatible.

To self-host, clone the repo, build it, and run it.

## Why use it

- **Your data, your device.** Sessions, notes, and transcripts are stored locally in SQLite. Attachments and recordings remain local files, and you can export Markdown when you need it.
- **Local transcription.** Transcription runs on-device, so audio never leaves your machine.
- **Bring your own AI.** Use any LLM provider, including OpenAI-compatible services and local models.
- **Open-source community layer, MIT.** Fork it, sell it, or self-host it.
- **Optional cloud features.** You can use local or bring-your-own-key workflows, or opt into hosted AI, encrypted CloudSync, and sharing when those fit your workflow.

## Name history

**anarlog** started as **Hyprnote**, then briefly used the **char** name.

We later split the work into two projects. **[char](https://char.com)** is the team's current productivity app. **anarlog** is this open-source, local-first meeting notetaker.

This repository is not the current char codebase, and anarlog is not being retired. Its community application stays MIT-licensed, forkable, self-hostable, and built for local notes you control.

If you came here from Granola, welcome. If you came here from Hyprnote, welcome back.

Either way, it's yours.

---

**License:** Community [MIT](LICENSE) · Enterprise [commercial](LICENSE.enterprise) · [Full boundary](LICENSING.md) · **Maintainers:** [fastrepl](https://github.com/fastrepl)
