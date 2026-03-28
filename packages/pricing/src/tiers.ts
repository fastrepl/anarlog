export type PlanTier = "free" | "lite" | "pro";

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
  features: Array<{
    label: string;
    included: boolean | "partial";
  }>;
}

export const PLAN_TIERS: PlanTierData[] = [
  {
    id: "free",
    name: "Free",
    price: "$0",
    period: "per month",
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
