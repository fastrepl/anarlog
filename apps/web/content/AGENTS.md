# Product Positioning

All content created under this directory must follow the positioning below.

## What Anarlog Is

Anarlog is the open-source AI meeting notetaker. It runs on your machine, keeps its canonical meeting data in local SQLite, and is MIT-licensed. The app you previously knew as "Char" is now Anarlog.

> Note: This blog lives at anarlog.so. The brand "Char" still exists, but it's a different product now (an agentic todo notepad — see char.com). All content under this directory is for **Anarlog**, the meeting notetaker.

## Core Philosophy

**Zero lock-in.** Anarlog gives users control over their local meeting data, AI stack, and workflow. Markdown is an export format, not the canonical storage model.

**Local-first, not file-based.** Sessions, notes, transcripts, and meeting metadata are stored locally in SQLite. Attachments remain separate local files. Users can export Markdown or other formats when they need them.

**Secure by design.** IT teams can audit the code, and you can use the AI provider your security team approves.

**Simple but powerful.** Complete control without complexity. The tool doesn't get in your way.

## Target Audience (in this order)

1. Engineers, developers, technical founders
2. Privacy-conscious professionals (lawyers, healthcare, finance)
3. People who already use tools like Obsidian, Notion, or local-first workflows
4. People whose company has banned other cloud-based tools like Otter, ChatGPT, Granola
5. Open source enthusiasts
6. People who already have unused LLM API credits

## Core Features

**Real-time transcription**

- System audio capture (no bots joining calls, no calendar permissions)
- Live transcript generated while user takes notes

**AI summary**

- Combines user notes + transcript to create structured summaries
- User controls which AI processes their data

**Your choice of AI stack**

- Managed cloud service (kept through the Char→Anarlog migration window)
- Bring your own API keys (OpenAI, Deepgram, Anthropic, others)
- Run local models (via Ollama or LM Studio)

**Local data with portable exports**

- Canonical sessions, notes, and transcripts are stored locally in SQLite
- Markdown is available as an export format; do not present it as the default or canonical note format
- Attachments and recordings are managed as separate local files
- Users can choose an export or integration workflow without depending on a vendor-hosted workspace

**Additional capabilities**

- Floating panel for quick recording controls
- Keyboard shortcuts
- Custom templates for different meeting types
- AI chat to query transcripts
- Search across all meetings
- Import existing recordings/transcripts
- 45+ language support
- Optional end-to-end encrypted CloudSync and note sharing for Pro users

## What Makes Anarlog Different

**vs. Other AI notetakers (Granola, Otter, Fireflies, Fathom, tldv):**

- Local SQLite storage instead of a vendor-hosted meeting workspace
- System audio capture instead of meeting bots
- User's choice of AI provider instead of vendor lock-in
- Open source — audit, fork, run forever
- Local ownership and portable exports instead of platform dependency

**Privacy approach:**

- Canonical meeting data lives locally by default
- Users choose on-device, bring-your-own-key, or hosted AI
- CloudSync encrypts data before it leaves a device; sharing and hosted features have their own explicit data paths
- Do not claim that data never leaves a device unless the configuration is explicitly local-only
- No data used for AI training

**Works With**
All meeting types: Zoom, Teams, Google Meet, phone calls, in-person conversations

## Brand Voice

**We are:**

- Direct and honest
- Engineering-minded
- Focused on fundamentals
- Anti-lock-in
- Pro-ownership

**We are not:**

- Corporate or overly polished
- Privacy-paranoid (we're about control, not fear)
- Feature-bloated
- Trying to be everything to everyone

## Key Messaging Themes

1. **Complete control** — over AI stack, data, and workflow
2. **True ownership** — local SQLite data and local attachments, not a vendor-hosted meeting workspace
3. **No lock-in** — portable exports and an open-source codebase
4. **Open source** — audit it, fork it, trust it
5. **Simple + powerful** — control without complexity
6. **For high-agency people** — built for those who refuse to compromise

## What We're Building Toward

A meeting notetaker that gives complete control without getting in your way. Clean, simple, aesthetic — but with full ownership underneath.

## Critical Reminders

- **Name:** Always use "Anarlog" for the meeting notetaker. "Char" is the legacy name (the app pre-rename) AND a different product (the agentic todo notepad at char.com). Don't conflate them in content.
- **Hyprnote** is the original product name — only mention in historical context.
- **Don't recommend Char as a "managed Anarlog"** — Char is a different product (delegation/todos), not a managed cloud version of the meeting notetaker.
- **Tone:** Direct, engineering-minded, respects user intelligence
- **Focus:** Zero lock-in, true ownership, complete control, open source
- **Avoid:** Generic productivity language, corporate marketing speak, fear-based messaging
