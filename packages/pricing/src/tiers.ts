export type PlanTier = "free" | "lite" | "pro";
export type PlanFeature = {
  label: string;
  included: boolean | "partial";
  tooltip?: string;
};

export type TierAction =
  | {
      label: string;
      style: "current" | "upgrade" | "downgrade";
      targetPlan: "lite" | "pro";
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
      { label: "Bring Your Own Key", included: true },
      { label: "Export to Various Formats", included: true },
      { label: "Local-first", included: true },
      { label: "Custom Default Folder", included: true },
      { label: "Templates", included: true },
      { label: "Shortcuts", included: true },
      { label: "Chat", included: true },
      { label: "Integrations", included: false },
      { label: "Cloud Services (STT & LLM)", included: false },
      { label: "Cloud Sync", included: false },
      { label: "Shareable Links", included: false },
    ],
  },
  {
    id: "lite",
    name: "Lite",
    price: "$8",
    period: "/month",
    subtitle: null,
    features: [
      { label: "Everything in Free", included: true },
      { label: "Cloud Services (STT & LLM)", included: true },
      { label: "Speaker Identification", included: "partial" },
      { label: "Advanced Templates", included: false },
      { label: "Cloud Sync", included: false },
      { label: "Shareable Links", included: false },
    ],
  },
  {
    id: "pro",
    name: "Pro",
    price: "$25",
    period: "/month",
    subtitle: "or $250/year",
    features: [
      { label: "Everything in Lite", included: true },
      { label: "Change Playback Rates", included: true },
      { label: "Advanced Templates", included: true },
      { label: "Integrations", included: true },
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
      { label: "Bring Your Own Key", included: true },
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
      { label: "Cloud Services (STT & LLM)", included: false },
      { label: "Speaker Identification", included: false },
    ],
  },
  {
    id: "lite",
    name: "Lite",
    price: {
      monthly: 8,
      yearly: null,
    },
    description:
      "Unlimited cloud transcription and AI models without the complexity. No API keys needed — just sign in and go.",
    features: [
      { label: "Everything in Free", included: true },
      { label: "Cloud Services (STT & LLM)", included: true },
      { label: "Speaker Identification", included: "partial" },
      { label: "Change Playback Rates", included: false },
      { label: "Integrations", included: false },
      { label: "Advanced Templates", included: false },
      { label: "Folders View", included: false },
      { label: "Cloud Sync", included: false },
      { label: "Shareable Links", included: false },
    ],
  },
  {
    id: "pro",
    name: "Pro",
    price: {
      monthly: 25,
      yearly: 250,
    },
    description:
      "Everything in Lite, plus advanced sharing and team features out of the box.",
    popular: true,
    features: [
      { label: "Everything in Lite", included: true },
      { label: "Change Playback Rates", included: true },
      {
        label: "Integrations",
        included: true,
        tooltip:
          "Google Calendar is available now. Additional integrations are in progress.",
      },
      { label: "Advanced Templates", included: "partial" },
      { label: "Folders View", included: "partial" },
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
  lite: 1,
  pro: 2,
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
    if (tierId === "lite" || tierId === "pro") {
      return {
        label: tierId === "lite" ? "Get Lite" : "Get Pro",
        style: "upgrade",
        targetPlan: tierId,
      };
    }
  }

  if (tierId === "free") {
    return null;
  }

  return {
    label:
      direction === "upgrade"
        ? `Upgrade to ${tierId === "pro" ? "Pro" : "Lite"}`
        : `Switch to ${tierId === "pro" ? "Pro" : "Lite"}`,
    style: direction,
    targetPlan: tierId,
  };
}
