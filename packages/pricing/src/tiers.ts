export type PlanTier = "free" | "pro";
export type PlanFeature = {
  label: string;
  included: boolean | "partial";
  tooltip?: string;
};

export type TierAction =
  | {
      label: string;
      style: "current" | "upgrade" | "downgrade";
      targetPlan: "pro";
    }
  | { label: string; style: "current"; targetPlan?: undefined }
  | null;

export interface PlanTierData {
  id: PlanTier;
  name: string;
  price: string;
  period: string;
  subtitle: string | null;
  features: PlanFeature[];
}

export const PLAN_TIERS: PlanTierData[] = [
  {
    id: "free",
    name: "Free",
    price: "$0",
    period: "/month",
    subtitle: null,
    features: [
      { label: "On-device Transcription", included: true },
      { label: "Save Audio Recordings", included: true },
      { label: "Audio Player", included: true },
      { label: "Bring Your Own Key (STT & LLM)", included: true },
      { label: "Export to Various Formats", included: true },
      { label: "Local-first", included: true },
      { label: "Custom Default Folder", included: true },
      { label: "Templates", included: true },
      { label: "Shortcuts", included: true },
      { label: "Chat", included: true },
      { label: "Connect to Google Calendar", included: false },
      { label: "Connect to Outlook Calendar", included: false },
      { label: "Cloud Transcription", included: false },
      { label: "Cloud LLM", included: false },
      { label: "Cloud Sync", included: false },
      { label: "Shareable Links", included: false },
    ],
  },
  {
    id: "pro",
    name: "Pro",
    price: "$15",
    period: "/month",
    subtitle: "or $150/year",
    features: [
      { label: "Everything in Free", included: true },
      { label: "Cloud Transcription", included: true },
      { label: "Cloud LLM", included: true },
      { label: "Speaker Identification", included: "partial" },
      { label: "Advanced Templates", included: true },
      { label: "Connect to Google Calendar", included: true },
      { label: "Connect to Outlook Calendar", included: true },
      { label: "Cloud Sync", included: "partial" },
      { label: "Shareable Links", included: "partial" },
    ],
  },
];

export interface MarketingPlanData {
  id: PlanTier;
  name: string;
  price: { monthly: number; yearly: number | null } | null;
  description: string;
  popular?: boolean;
  features: PlanFeature[];
}

export const MARKETING_PLAN_TIERS: MarketingPlanData[] = [
  {
    id: "free",
    name: "Free",
    price: null,
    description:
      "Fully functional with your own API keys. Perfect for individuals who want complete control.",
    features: [
      { label: "On-device Transcription", included: true },
      { label: "Save Audio Recordings", included: true },
      { label: "Audio Player", included: true },
      { label: "Bring Your Own Key (STT & LLM)", included: true },
      { label: "Export to Various Formats", included: true },
      {
        label: "Custom Default Folder",
        included: true,
        tooltip: "Move your default folder location to anywhere you prefer.",
      },
      { label: "Chat", included: true },
      { label: "Contacts View", included: true },
      { label: "Calendar View", included: true },
      { label: "Templates", included: true },
      { label: "Transcript Editor", included: "partial" },
      { label: "Shortcuts", included: "partial" },
      { label: "Cloud Transcription", included: false },
      { label: "Cloud LLM", included: false },
      { label: "Speaker Identification", included: false },
    ],
  },
  {
    id: "pro",
    name: "Pro",
    price: {
      monthly: 15,
      yearly: 150,
    },
    description:
      "Hosted transcription and AI models, speaker identification, calendar connections, and advanced workflow features.",
    popular: true,
    features: [
      { label: "Everything in Free", included: true },
      { label: "Cloud Transcription", included: true },
      { label: "Cloud LLM", included: true },
      { label: "Speaker Identification", included: "partial" },
      { label: "Connect to Google Calendar", included: true },
      { label: "Connect to Outlook Calendar", included: true },
      { label: "Advanced Templates", included: "partial" },
      {
        label: "Connect to OpenClaw",
        included: "partial",
        tooltip: "Select which notes to sync",
      },
      {
        label: "Cloud Sync",
        included: "partial",
        tooltip: "Select which notes to sync",
      },
      {
        label: "Shareable Links",
        included: "partial",
        tooltip: "DocSend-like: view tracking, expiration, revocation",
      },
    ],
  },
];

export const TIER_ORDER: Record<PlanTier, number> = {
  free: 0,
  pro: 1,
};

export function getActionForTier(
  tierId: PlanTier,
  currentPlan: PlanTier,
  canStartTrial: boolean,
): TierAction {
  if (tierId === currentPlan) {
    return { label: "Current plan", style: "current" };
  }

  const direction =
    TIER_ORDER[tierId] > TIER_ORDER[currentPlan] ? "upgrade" : "downgrade";

  if (currentPlan === "free") {
    if (tierId === "pro" && canStartTrial) {
      return {
        label: "Start free trial",
        style: "upgrade",
        targetPlan: "pro",
      };
    }
    return {
      label: "Get Pro",
      style: "upgrade",
      targetPlan: "pro",
    };
  }

  if (tierId === "free") {
    return null;
  }

  return {
    label: direction === "upgrade" ? "Upgrade to Pro" : "Switch to Pro",
    style: direction,
    targetPlan: tierId,
  };
}
