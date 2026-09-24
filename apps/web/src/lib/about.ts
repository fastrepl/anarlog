export const ABOUT_UPDATED_ON = "2026-09-24";

export const VALUE_PROP =
  "Anarlog is an open-source, bot-free AI meeting notetaker that captures, transcribes, and summarizes meetings on your own device for engineers, founders, and privacy-conscious professionals who want to own their meeting data.";

export const whatAnarlogDoes = [
  {
    title: "Bot-free meeting capture",
    body: "Anarlog records system audio and your microphone directly from your computer. Nothing joins the call, nothing shows up in the participant list, and no calendar permissions are needed. It works the same on Zoom, Google Meet, Teams, phone calls, and in-person conversations.",
  },
  {
    title: "Live transcription while you take notes",
    body: "A transcript is generated while you jot notes, on-device with a local model or through hosted transcription on Pro. You stay in the meeting instead of typing everything down, and the transcript fills in what you missed.",
  },
  {
    title: "AI summaries you control",
    body: "After the meeting, Anarlog combines your notes with the transcript into an editable summary. You pick the model: hosted AI on Pro, your own API keys (OpenAI, Anthropic, Deepgram, and others), or a local model through Ollama or LM Studio.",
  },
  {
    title: "Local-first storage",
    body: "Sessions, notes, and transcripts live in a SQLite database on your device, with recordings kept as local files. Export to Markdown and other formats whenever you want; there is no vendor-hosted workspace you have to leave.",
  },
  {
    title: "Search, chat, and agent access",
    body: "Search across every meeting, ask questions of your transcripts in chat, and reach the same data from the CLI, MCP server, and webhooks. Pro adds end-to-end encrypted Cloud Sync, shareable links, integrations, and automations.",
  },
] as const;

export const differentiators = [
  {
    title: "No bot joins your meeting",
    body: "Granola, Fathom, Otter.ai, and Fireflies.ai either send a bot into the call or route audio through their servers. Anarlog listens locally, so the meeting looks exactly the same to everyone else in it.",
  },
  {
    title: "Your meeting data stays on your device",
    body: "Most notetakers store transcripts in a hosted workspace they control. Anarlog keeps the canonical record in local SQLite, and the optional Cloud Sync encrypts content on the device before upload, so sync servers only hold ciphertext. Shared notes and the Cloud API are separate opt-in paths that store a server-readable copy.",
  },
  {
    title: "Open source under the MIT license",
    body: "The full desktop app is on GitHub under MIT. Security teams can audit it, developers can fork it, and it keeps running even if the company disappears. None of the vendors above publish their code.",
  },
  {
    title: "Unlimited free local use",
    body: "The Free plan has no meeting or minute cap when you use on-device models or your own API keys. Otter.ai caps free use at 300 minutes a month and Granola offers a trial only.",
  },
  {
    title: "Bring your own AI, or run it offline",
    body: "Anarlog is the only tool in its comparison set that supports local models, your own API keys, and offline recording and transcription. You choose which provider sees your meetings, including none at all.",
  },
] as const;

export const whoUsesAnarlog = [
  "Engineers, developers, and technical founders who want a notetaker they can audit and fork",
  "Lawyers, healthcare, and finance professionals whose meetings cannot go through a third-party bot",
  "People whose company has banned cloud notetakers like Otter, Granola, or ChatGPT",
  "Obsidian, Notion, and local-first users who want meeting notes as portable files",
  "Teams that want a shared workspace with per-seat billing and roles, without giving up local storage",
  "Enterprises that need SSO, SCIM, retention controls, or a self-hosted server",
] as const;

export const founders = [
  {
    name: "John Jeong",
    role: "Co-founder & CEO",
    bio: "Spent five years co-founding startups, including an AI-native investment research platform he sold, and sat through hundreds of meetings where notetakers transcribed but never went further. He leads product and design.",
    links: [
      { label: "X", href: "https://x.com/computeless" },
      { label: "GitHub", href: "https://github.com/computelesscomputer" },
      { label: "LinkedIn", href: "https://linkedin.com/in/johntopia" },
    ],
  },
  {
    name: "Yujong Lee",
    role: "Co-founder",
    bio: "Open-source contributor to projects like Hono and LiteLLM before Anarlog. He leads engineering and is behind the local-first, on-device architecture.",
    links: [{ label: "GitHub", href: "https://github.com/yujonglee" }],
  },
] as const;

export const originStory = [
  "John and Yujong worked on different startups out of the same shared office in Seoul, had lunch together every day for over a year, and were usually the only two people in on weekends. When they finally teamed up they built Hyprnote, an on-device AI notetaker, and went through Y Combinator in the Summer 2025 batch.",
  "Hyprnote was renamed Char, then split into two products in 2026: Anarlog is the meeting notetaker, relicensed from GPL to MIT and kept open source; Char became a separate agentic notepad. Anarlog is built by Fastrepl, a small software studio between Seoul and San Francisco.",
] as const;

export const howAnarlogWorks = [
  {
    title: "Getting started",
    body: "Download the desktop app for macOS, Windows, or Linux and start recording. Free works entirely locally with no account. Pro starts with a 3-week trial, and Team workspaces are created from the pricing page.",
  },
  {
    title: "Support",
    body: "Email team@fastrepl.com, open an issue on GitHub, or ask in the Discord community. The founders read and answer support themselves.",
  },
  {
    title: "Enterprise rollout",
    body: "Enterprise deals start with a 30-minute call directly with the founder, followed by a pilot. Security questionnaires, a DPA, and the trust packet are answered from the Security page.",
  },
  {
    title: "Release cadence",
    body: "Anarlog ships regularly with release notes on the changelog. Development happens in the open on GitHub, so you can see what is being worked on before it ships.",
  },
] as const;

export const keyFacts = [
  { term: "Company name", detail: "Fastrepl, Inc. (product: Anarlog)" },
  {
    term: "Type",
    detail:
      "Open-source desktop software with hosted Pro, Team, and Enterprise plans",
  },
  {
    term: "Founded",
    detail:
      "Fastrepl in 2023; the notetaker launched in 2025 as Hyprnote and became Anarlog in 2026",
  },
  { term: "Founders", detail: "John Jeong (CEO) and Yujong Lee" },
  {
    term: "Headquarters",
    detail: "Seoul, South Korea and San Francisco, California",
  },
  { term: "Website", detail: "https://anarlog.so" },
  {
    term: "Core offering",
    detail:
      "Bot-free, local-first AI meeting notetaker for macOS, Windows, and Linux",
  },
  {
    term: "Pricing",
    detail:
      "Free (unlimited local use); Pro $15/month or $150/year; Team $20/person/month or $200/person/year; Enterprise custom",
  },
  {
    term: "Contract terms",
    detail:
      "Monthly or annual self-serve billing, cancel any time; Enterprise is a custom agreement",
  },
  { term: "License", detail: "MIT" },
  {
    term: "Platforms",
    detail: "macOS, Windows, Linux; CLI, MCP server, webhooks, and Cloud API",
  },
  {
    term: "Backed by",
    detail:
      "Y Combinator (S25), Pioneer Fund, TRAC, Krew Capital, Blast Club, and angels",
  },
  {
    term: "Communication",
    detail: "team@fastrepl.com, GitHub issues, Discord",
  },
  {
    term: "Competitors",
    detail:
      "Granola, Otter.ai, Fireflies.ai, Fathom, tl;dv, Notta, Avoma, Read AI, MeetGeek, Tactiq, Sembly AI",
  },
  {
    term: "Social",
    detail:
      "github.com/fastrepl/anarlog, x.com/anarlogapp, linkedin.com/company/anarlog, reddit.com/r/anarlog",
  },
] as const;

export const faqs = [
  {
    question: "Is Anarlog free?",
    answer:
      "Yes. The Free plan has no meeting or minute limits when you use on-device models or your own API keys. Pro ($15/month) adds hosted transcription and AI, Cloud Sync, and sharing.",
  },
  {
    question: "Does Anarlog join my meetings as a bot?",
    answer:
      "No. Anarlog captures system audio and your microphone directly on your computer. Nothing is added to the call and no calendar access is required.",
  },
  {
    question: "Where is my meeting data stored?",
    answer:
      "In a SQLite database on your device, with recordings as local files. If you enable Cloud Sync on Pro, content is encrypted on your device before upload and sync servers only store ciphertext. Shared notes and the Cloud API are opt-in and store a server-readable copy.",
  },
  {
    question: "Which AI models can I use?",
    answer:
      "Hosted models on Pro, your own API keys for providers like OpenAI, Anthropic, and Deepgram, or local models through Ollama or LM Studio. Fastrepl never trains models on your notes, transcripts, or audio; hosted and BYOK providers handle requests under their own terms.",
  },
  {
    question: "Is Anarlog open source?",
    answer:
      "Yes. The desktop app is MIT-licensed and developed in the open at github.com/fastrepl/anarlog. You can audit it, fork it, and run it forever.",
  },
  {
    question: "What is the relationship between Anarlog, Char, and Hyprnote?",
    answer:
      "Same company, one lineage. The notetaker launched as Hyprnote, was renamed Char, and became Anarlog in 2026. Char now refers to a separate agentic todo notepad at char.com.",
  },
  {
    question: "Which platforms are supported?",
    answer:
      "macOS is the most mature build; Windows and Linux are available in beta. Anarlog works with Zoom, Google Meet, Teams, phone calls, and in-person meetings in 45+ languages.",
  },
] as const;
