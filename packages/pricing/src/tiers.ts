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
  features: string[];
  notIncluded: string[];
}

export const PLAN_TIERS: PlanTierData[] = [
  {
    id: "free",
    name: "Free",
    price: "$0",
    period: "",
    subtitle: null,
    features: ["On-device transcription", "Bring your own API keys", "Export"],
    notIncluded: ["Cloud AI", "Integrations"],
  },
  {
    id: "lite",
    name: "Lite",
    price: "$8",
    period: "/mo",
    subtitle: "Monthly only",
    features: ["Everything in Free", "Cloud AI (STT & LLM)", "Integrations"],
    notIncluded: ["Advanced templates", "Cloud sync"],
  },
  {
    id: "pro",
    name: "Pro",
    price: "$25",
    period: "/mo",
    subtitle: "or $250/year",
    features: [
      "Everything in Free",
      "Cloud AI (STT & LLM)",
      "Advanced templates",
      "Integrations",
      "Cloud sync",
      "Shareable links",
    ],
    notIncluded: [],
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
