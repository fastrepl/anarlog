// Competitor pricing drifts. Every figure here must be re-checked against the
// vendor's own pricing page and PRICING_VERIFIED_ON bumped when it is.
export const PRICING_VERIFIED_ON = "2026-08-11";

export type ComparisonRow = {
  name: string;
  // Square logo under public/icons/. Rows without one fall back to name-only.
  icon?: string;
  url: string;
  paidFrom: string;
  freeTier: string;
  capture: string;
  dataLocation: string;
  openSource: boolean;
  ownKeys: boolean;
};

export const ANARLOG_ROW: ComparisonRow = {
  name: "Anarlog",
  icon: "/icon-192x192.png",
  url: "/",
  paidFrom: "$15/mo",
  freeTier: "Unlimited local transcription",
  capture: "Desktop audio",
  dataLocation: "Local SQLite",
  openSource: true,
  ownKeys: true,
};

export const COMPARISON_ROWS: ComparisonRow[] = [
  {
    name: "Otter.ai",
    icon: "/icons/otter.png",
    url: "https://otter.ai/",
    paidFrom: "$16.99/user/mo",
    freeTier: "300 min/mo",
    capture: "Meeting bot",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Fireflies.ai",
    icon: "/icons/fireflies.png",
    url: "https://fireflies.ai/",
    paidFrom: "$18/mo",
    freeTier: "Limited transcription",
    capture: "Bot or desktop",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Fathom",
    icon: "/icons/fathom.png",
    url: "https://www.fathom.ai/pricing",
    paidFrom: "$19/mo",
    freeTier: "Unlimited recording",
    capture: "Bot or desktop",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Granola",
    icon: "/icons/granola.png",
    url: "https://www.granola.ai/pricing",
    paidFrom: "$14/mo",
    freeTier: "Trial only",
    capture: "Desktop audio",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "tl;dv",
    icon: "/icons/tldv.png",
    url: "https://tldv.io/",
    paidFrom: "$29/user/mo",
    freeTier: "Unlimited recording",
    capture: "Meeting bot",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Notta",
    icon: "/icons/notta.png",
    url: "https://www.notta.ai/en",
    paidFrom: "$13.49/user/mo",
    freeTier: "Limited transcription",
    capture: "Bot or files",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Avoma",
    icon: "/icons/avoma.png",
    url: "https://www.avoma.com/pricing",
    paidFrom: "$19/mo",
    freeTier: "Limited",
    capture: "Meeting bot",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Read AI",
    icon: "/icons/readai.png",
    url: "https://www.read.ai",
    paidFrom: "$19.75/mo",
    freeTier: "5 transcripts/mo",
    capture: "Meeting bot",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "MeetGeek",
    icon: "/icons/meetgeek.png",
    url: "https://meetgeek.ai/",
    paidFrom: "$19/user/mo",
    freeTier: "3 hrs/mo",
    capture: "Meeting bot",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Tactiq",
    icon: "/icons/tactiq.png",
    url: "https://tactiq.io/buy",
    paidFrom: "$12/user/mo",
    freeTier: "10 meetings",
    capture: "Chrome extension",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Sembly AI",
    icon: "/icons/sembly.png",
    url: "https://www.sembly.ai/",
    paidFrom: "$15/user/mo",
    freeTier: "60 min/mo",
    capture: "Meeting bot",
    dataLocation: "Vendor cloud",
    openSource: false,
    ownKeys: false,
  },
  {
    name: "Meetily",
    icon: "/icons/meetily.png",
    url: "https://github.com/Zackriya-Solutions/meetily",
    paidFrom: "$10/mo",
    freeTier: "Unlimited local transcription",
    capture: "Desktop audio",
    dataLocation: "Local",
    openSource: true,
    ownKeys: true,
  },
];
