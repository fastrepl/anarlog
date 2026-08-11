// Competitor rows make public claims about named products. Every field here
// must be checkable against the vendor's own published material; bump
// PRICING_VERIFIED_ON whenever a row is re-checked.
//
// Column definitions, kept narrow so each boolean stays defensible:
//   botFree      - can capture without adding a participant to the call
//   localData    - meeting record is stored on your device by default
//   offline      - can record and transcribe with no internet connection
//   localModels  - can run local STT/LLM models such as Ollama or LM Studio
//   ownKeys      - can supply your own AI provider API keys
//   openSource   - source is publicly licensed
export const PRICING_VERIFIED_ON = "2026-08-11";

export type ComparisonRow = {
  name: string;
  // Square logo under public/icons/. Rows without one fall back to name-only.
  icon?: string;
  url: string;
  paidFrom: string;
  freeTier: string;
  botFree: boolean;
  localData: boolean;
  offline: boolean;
  localModels: boolean;
  ownKeys: boolean;
  openSource: boolean;
};

export const ANARLOG_ROW: ComparisonRow = {
  name: "Anarlog",
  icon: "/icon-192x192.png",
  url: "/",
  paidFrom: "$15/mo",
  freeTier: "Unlimited local",
  botFree: true,
  localData: true,
  offline: true,
  localModels: true,
  ownKeys: true,
  openSource: true,
};

export const COMPARISON_ROWS: ComparisonRow[] = [
  {
    name: "Otter.ai",
    icon: "/icons/otter.png",
    url: "https://otter.ai/",
    paidFrom: "$16.99/user/mo",
    freeTier: "300 min/mo",
    botFree: true,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "Fireflies.ai",
    icon: "/icons/fireflies.png",
    url: "https://fireflies.ai/",
    paidFrom: "$18/mo",
    freeTier: "Limited",
    botFree: true,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "Fathom",
    icon: "/icons/fathom.png",
    url: "https://www.fathom.ai/pricing",
    paidFrom: "$19/mo",
    freeTier: "Unlimited recording",
    botFree: true,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "Granola",
    icon: "/icons/granola.png",
    url: "https://www.granola.ai/pricing",
    paidFrom: "$14/mo",
    freeTier: "Trial only",
    botFree: true,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "tl;dv",
    icon: "/icons/tldv.png",
    url: "https://tldv.io/",
    paidFrom: "$29/user/mo",
    freeTier: "Unlimited recording",
    botFree: false,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "Notta",
    icon: "/icons/notta.png",
    url: "https://www.notta.ai/en",
    paidFrom: "$13.49/user/mo",
    freeTier: "Limited",
    botFree: true,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "Avoma",
    icon: "/icons/avoma.png",
    url: "https://www.avoma.com/pricing",
    paidFrom: "$19/mo",
    freeTier: "Limited",
    botFree: false,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "Read AI",
    icon: "/icons/readai.png",
    url: "https://www.read.ai",
    paidFrom: "$19.75/mo",
    freeTier: "5 transcripts/mo",
    botFree: false,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "MeetGeek",
    icon: "/icons/meetgeek.png",
    url: "https://meetgeek.ai/",
    paidFrom: "$19/user/mo",
    freeTier: "3 hrs/mo",
    botFree: false,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "Tactiq",
    icon: "/icons/tactiq.png",
    url: "https://tactiq.io/buy",
    paidFrom: "$12/user/mo",
    freeTier: "10 meetings",
    botFree: true,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
  {
    name: "Sembly AI",
    icon: "/icons/sembly.png",
    url: "https://www.sembly.ai/",
    paidFrom: "$15/user/mo",
    freeTier: "60 min/mo",
    botFree: false,
    localData: false,
    offline: false,
    localModels: false,
    ownKeys: false,
    openSource: false,
  },
];
