export const TRUST_CENTER_UPDATED_ON = "2026-08-29";

export const securityReviewAnswers = [
  {
    question: "How is meeting content encrypted?",
    summary:
      "Notes live in local SQLite on the device. Cloud Sync encrypts content on the device before upload; Fastrepl only stores ciphertext plus operational metadata.",
    detail:
      "The desktop app stores notes, transcripts, and meeting metadata in a local SQLite database. Optional Cloud Sync encrypts that content on the device with keys derived from a recovery key that stays in the operating-system keychain and never reaches Fastrepl. Sync servers see encrypted records plus account and workspace identifiers, timestamps, sizes, and device names — not titles or note content. Data in transit uses HTTPS/TLS. Shared notes and Cloud API & Connectors are separate, opt-in paths that store a server-readable copy so a recipient or agent can open the note.",
  },
  {
    question: "Where does data live, and which jurisdiction applies?",
    summary:
      "Canonical meeting data stays on employee devices. Fastrepl-operated cloud services run in the United States. A customer-hosted data plane is the path for a different region.",
    detail:
      "Local notes never leave the device unless a user enables a cloud feature. Fastrepl-operated cloud — accounts, optional Cloud Sync ciphertext, shared notes, and hosted AI/transcription gateways — currently runs in the United States. Customer-hosted capture and a customer-controlled data plane are the way to keep meeting infrastructure in a region you choose. We do not offer a certified-cloud EU SKU today.",
  },
  {
    question: "How long do you keep data?",
    summary:
      "Local data stays until the user deletes it. Cloud data we control is deleted within 30 days of account deletion. Audio uploaded for cloud transcription is deleted when the job finishes.",
    detail:
      "Local notes, transcripts, and recordings stay on the device until the user removes them. Audio retention on the desktop is a user setting (don't save, 1 day, 3 days, 1 week, 1 month, or forever). Cloud Sync records, transcription results, shared notes, and Cloud API copies are deleted within 30 days of account deletion unless the law requires a longer hold. Audio uploaded for cloud transcription is deleted from our storage once the job completes. Cloud API copies are deleted when the feature is turned off. Shared notes remain available until sharing stops or the account is deleted.",
  },
  {
    question: "Do you train models on our meetings?",
    summary:
      "Fastrepl does not train models on notes, transcripts, audio, or connected calendar data. Review the selected AI provider if you use hosted or bring-your-own-key models.",
    detail:
      "Fastrepl does not use notes, transcripts, audio, or connected calendar data to train AI models, and we do not sell that data. If a team uses on-device transcription and a local language model, meeting content never leaves the device for AI. If they use Anarlog Cloud or a bring-your-own-key provider, the audio or text needed for that request goes to the selected provider under that provider's terms — review that provider's retention and training policy before sending sensitive meetings.",
  },
  {
    question: "Who are your subprocessors?",
    summary:
      "Hosting, encrypted sync storage, payments, analytics, and optional speech-to-text or AI providers. None of them join the meeting.",
    detail:
      "The current processor list lives in the table below and in the privacy policy. Providers receive only what the enabled feature needs. Cloud Sync storage holds ciphertext. Speech-to-text and model providers receive content only when a user selects a hosted or bring-your-own-key route. Analytics and error tools are designed not to include meeting audio, transcripts, notes, or summaries.",
  },
  {
    question: "Does a bot join our calls?",
    summary:
      "No. The desktop app captures microphone and system audio locally. Nothing is added to the participant list.",
    detail:
      "Anarlog does not send a meeting bot into Zoom, Google Meet, Microsoft Teams, or other calls. Capture happens on the desktop from microphone and system audio. That is the product, not a setting. Customer-hosted Meet or Zoom workers, when used by an enterprise pilot, are a separate capture path and are disclosed as such — they are not the default desktop product.",
  },
];

export const architectureLayers = [
  {
    title: "Employee devices",
    body: "Canonical notes, transcripts, recordings, and most settings live in local SQLite. The MIT-licensed desktop client is auditable.",
  },
  {
    title: "Optional Cloud Sync",
    body: "End-to-end encrypted replicas so other signed-in devices stay current. Fastrepl cannot read the recovery key or the note content.",
  },
  {
    title: "Optional AI and transcription",
    body: "On-device models, bring-your-own keys, or Anarlog Cloud. Content goes only to the provider the user selected for that request.",
  },
  {
    title: "Optional sharing and Cloud API",
    body: "A server-readable copy exists only when someone shares a note or turns on Cloud API & Connectors. Turning the API off deletes those copies.",
  },
];

export const subprocessors = [
  {
    name: "Fly.io, Netlify, Supabase, Render, Amazon Web Services, Cloudflare",
    purpose: "Hosting, authentication, storage, and downloads",
    receives:
      "Account data, operational metadata, and any server-side records created by an enabled cloud feature",
  },
  {
    name: "SQLite Cloud",
    purpose: "Cloud Sync storage",
    receives: "End-to-end encrypted sync records plus operational metadata",
  },
  {
    name: "Nango",
    purpose: "Calendar and connected-account integrations",
    receives:
      "Encrypted OAuth tokens and the calendar or issue-tracker data needed for a connected feature",
  },
  {
    name: "Stripe",
    purpose: "Payments",
    receives: "Billing details; Fastrepl never stores card numbers",
  },
  {
    name: "PostHog, Google Analytics, Microsoft Clarity",
    purpose: "Product and website analytics",
    receives:
      "Pseudonymous usage events, page views, and optional website session replay — not meeting audio, transcripts, notes, or summaries",
  },
  {
    name: "Sentry, Honeycomb",
    purpose: "Error monitoring and observability",
    receives:
      "Sanitized diagnostics and crash reports with meeting content stripped",
  },
  {
    name: "Deepgram, Soniox, AssemblyAI, Gladia, ElevenLabs, Fireworks AI, OpenAI, Mistral, Alibaba Cloud",
    purpose: "Optional cloud transcription",
    receives:
      "Audio for a transcription job when a user selects a hosted speech-to-text route",
  },
  {
    name: "OpenRouter, routing to providers such as Anthropic, Google, and Mistral",
    purpose: "Optional cloud AI",
    receives:
      "The text and instructions needed for a summary or chat request the user starts",
  },
  {
    name: "Exa, Jina",
    purpose: "Optional web search from AI chat",
    receives: "Search queries when a user enables web search in chat",
  },
  {
    name: "Loops",
    purpose: "Email",
    receives:
      "Email address and the content of transactional or marketing messages",
  },
];

export const retentionRows = [
  {
    item: "Local notes, transcripts, and recordings",
    retention: "On the device until the user deletes them",
  },
  {
    item: "Desktop audio files",
    retention:
      "User-selected: don't save, 1 day, 3 days, 1 week, 1 month, or forever",
  },
  {
    item: "Cloud Sync, transcription results, shared notes, Cloud API copies",
    retention:
      "Deleted within 30 days of account deletion, unless the law requires a hold",
  },
  {
    item: "Audio uploaded for cloud transcription",
    retention: "Deleted from Fastrepl storage when the job completes",
  },
  {
    item: "Cloud API & Connectors copies",
    retention: "Deleted when the feature is turned off",
  },
];

export const certificationStatus = {
  claimed: [] as string[],
  planned:
    "SOC 2, ISO 27001, AIUC-1, and HIPAA/BAA programs are planned after we have operational evidence. This page does not claim any of them.",
  hostedTrustCenter:
    "A hosted trust center — the Vanta or Oneleet surface buyers expect — comes after those programs start. It will be a separate site, not this page.",
};

export const contractualDocs = [
  {
    label: "Privacy Policy",
    href: "/privacy/",
    note: "What we collect, local-first defaults, and processor list",
  },
  {
    label: "Terms of Service",
    href: "/terms/",
    note: "Contract for using Anarlog",
  },
  {
    label: "Data Processing Addendum",
    href: "mailto:founders@anarlog.so?subject=Anarlog%20DPA",
    note: "Available on request for enterprise evaluations",
  },
];

export const shipsToday = [
  "Desktop app on macOS, with Windows and Linux in beta",
  "Bot-free local capture from microphone and system audio",
  "Local SQLite as the source of truth, plus Markdown and other exports",
  "On-device transcription and local models, or bring-your-own API keys",
  "Optional end-to-end encrypted Cloud Sync",
  "Optional sharing and Cloud API & Connectors, each with a distinct data path",
];

export const shipsWithPartners = [
  "Team workspaces, domain SSO, and SCIM provisioning",
  "Org-wide sharing, retention, and consent policies",
  "Customer-hosted capture and a customer-controlled data plane",
];

export const pilotSteps = [
  {
    title: "Security review",
    body: "Forward this site to IT, security, and legal. The security page, privacy policy, and source-visible client are the packet. We answer questionnaires from the same facts — we will not invent certifications.",
  },
  {
    title: "Scoped pilot",
    body: "A founder-led trial with a named team, a defined data boundary (local-only, encrypted sync, or customer-hosted capture), and a success check you choose. No SDR queue.",
  },
  {
    title: "Rollout",
    body: "Expand seats and policies after the pilot. Workspace admin, SSO/SCIM, or a customer-hosted data plane land with the teams that need them — not as a surprise bot in every meeting.",
  },
];

export const proofStatus = {
  headline: "Named customer proof is not published yet",
  body: "We work directly with early enterprise partners. We will only publish a named or properly anonymized outcome after that partner approves the company context, constraint, deployment mode, and result. This page does not invent metrics or logos.",
};
