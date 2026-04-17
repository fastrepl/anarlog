<!-- TODO: replace with Char 1.1 hero image -->
![twitter-image](https://github.com/user-attachments/assets/b6161cfd-ddfa-4c09-9fbb-ab5a2d6961fc)

<p align="center">
  <p align="center">Char — AI <strong>daily notes</strong> that remember and act</p>
  <p align="center">
   <a href="https://deepwiki.com/fastrepl/char"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
   <a href="https://char.com/discord" target="_blank"><img src="https://img.shields.io/static/v1?label=Join%20our&message=Discord&color=blue&logo=Discord" alt="Discord"></a>
   <a href="https://x.com/getcharnotes" target="_blank"><img src="https://img.shields.io/static/v1?label=Follow%20us%20on&message=X&color=black&logo=x" alt="X"></a>
  </p>
</p>

## What is Char?

Char is a daily notes app that listens to your meetings, reads your emails, and watches what you work on. It pulls out tasks and delegates them to AI agents or your team.

One note per day. Context that compounds.

- **Captures** meetings, emails, and screen activity — no bots, no audio leaves your device
- **Extracts** action items from everything that happened today
- **Delegates** tasks to AI agents (Claude, Cursor, Devin) or the right person on your team
- **Open source** and local-first — markdown files, your data stays yours

> *Week one it helps. Month one it knows you.*

<!-- TODO: add Char 1.1 daily note overview screenshot -->

> **Just want a local-first, on-device AI meeting notetaker?** Check out [Unsigned Char](https://github.com/fastrepl/unsigned-char) — a free standalone notetaker with speaker identification. No daily notes, no task delegation. Just transcription and summaries, fully local.

## Installation

```bash
brew install --cask fastrepl/fastrepl/char
```

- [macOS](https://char.com/download) (public beta)
- [Windows](https://github.com/fastrepl/char/issues/66) (q2 2026)
- [Linux](https://github.com/fastrepl/char/issues/67) (q2 2026)

## The Three Pillars

```
Capture → Extract → Delegate
```

Everything you do in a day flows through one note. Meetings, emails, screen activity, quick thoughts — it all stacks up. Action items get surfaced. Tasks get routed. Every day adds to the memory.

<!-- TODO: add pillars diagram / animated flow -->

## Highlights

### Daily Note

One timeline for your day. Meetings, emails, screen activity, and quick thoughts all flow into a single note that evolves as the day goes on.

<!-- TODO: add daily note screenshot (Char 1.1) -->

### Meeting Capture

Char listens directly to sounds coming in and out of your computer — no bots joining your calls. Transcripts, summaries, and action items appear in your daily note automatically.

<!-- TODO: add meeting capture screenshot (Char 1.1) -->

### Email Triage

Char reads your inbox, separates what matters from what doesn't, and pulls real action items into your daily note. Inbox zero as a side effect.

<!-- TODO: add email triage screenshot -->

### Screen Activity

Char notices what you're actually working on — the doc you're writing, the PR you're reviewing, the thread you're deep in. That context feeds back into your daily note so nothing gets lost between tabs.

<!-- TODO: add screen activity screenshot -->

### Task Delegation

Action items don't just sit on a list. Delegate them to AI agents (Claude, Cursor, Devin) or route them to the right person on your team — directly from your daily note.

<!-- TODO: add task delegation screenshot -->

### Realtime Transcript

While you stay engaged in the conversation, Char captures every detail so you don't have to type frantically.

<img width="688" height="568" alt="Realtime transcript" src="https://github.com/user-attachments/assets/e63ce73f-1a5f-49ce-a14d-dd8ba161e5bc" />

### From Memos to Summaries

Once the meeting is over, Char crafts a personalized summary based on your memos — though memos aren't mandatory. Char will still produce great summaries without your notes.

![offline enhancing-1](https://github.com/user-attachments/assets/13af787b-2f6e-4877-b90f-719edc45fb75)

### Truly Local

Char runs fully offline. Set up LM Studio or Ollama to operate Char in air-gapped environments — no internet required.

<img width="780" height="585" alt="no-wifi" src="https://github.com/user-attachments/assets/ecf08a9e-3b6c-4fb6-ab38-0bc572f54859" />

> **Note on accounts:** During onboarding, Char creates an account so you can experience the full product — including cloud-powered transcription and summarization — at its best quality. All your notes, transcripts, and data are stored locally on your machine in a local SQLite database. If you prefer not to keep an account, you can request deletion anytime at [char.com/app/account](https://char.com/app/account). Char will continue to work fully offline with a local LLM.

### Bring Your Own LLM

Prefer something custom? Swap in your own language model:

- Run local models via Ollama or LM Studio
- Use approved third-party APIs like Gemini, Claude, or Azure-hosted GPT
- Stay compliant with whatever your org allows

Char plays nice with whatever stack you're running.

<img width="912" height="712" alt="BYO LLM settings" src="https://github.com/user-attachments/assets/a6552c99-acbc-4d47-9d21-7f1925989344" />

### Note Templates

Prefer a certain style? Choose from predefined templates like bullet points, agenda-based, or paragraph summary. Or create your own.

Check out our [template gallery](https://char.com/templates) and add your own [here](https://github.com/fastrepl/char/tree/main/apps/web/content/templates).

### AI Chat

Ask follow-ups right inside your notes:

- "What were the action items?"
- "Rewrite this in simpler language"
- "Translate to Spanish"

<img width="959" height="712" alt="AI chat" src="https://github.com/user-attachments/assets/52b7dc14-906f-445f-91f9-b0089d40a495" />

### Integrations

- Apple Calendar, Contacts
- Obsidian
- Coming soon: Notion, Slack, Hubspot, Salesforce

<img width="912" height="712" alt="Integrations" src="https://github.com/user-attachments/assets/ab559e54-fda5-4c8c-97d7-ba1b9d134cc8" />
