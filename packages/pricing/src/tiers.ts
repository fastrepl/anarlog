export type PlanTier = "free" | "pro";
export type MarketingPlanTier = PlanTier | "team" | "enterprise";
export type PlanFeature = {
  label: string;
  included: boolean;
  tooltip?: string;
  availability?: "comingSoon";
};

export type MarketingPlanPrice =
  | { kind: "free" }
  | {
      kind: "fixed";
      monthly: number;
      yearly: number | null;
      billingUnit?: "person";
    }
  | { kind: "custom" };

// Behavioral variants only: consumers render their own (translated) labels,
// so no navigation or analytics decision can depend on display copy.
export type TierAction =
  | { kind: "current" }
  | { kind: "startTrial"; plan: "pro" }
  | { kind: "checkout"; plan: "pro"; direction: "upgrade" | "downgrade" }
  | null;

export interface PlanTierData {
  id: MarketingPlanTier;
  name: string;
  price: string;
  period: string;
  subtitle: string | null;
  description: string;
  features: PlanFeature[];
}

export interface MarketingPlanData {
  id: MarketingPlanTier;
  name: string;
  price: MarketingPlanPrice;
  description: string;
  popular?: boolean;
  features: PlanFeature[];
}

export const MARKETING_PLAN_TIERS: MarketingPlanData[] = [
  {
    id: "free",
    name: "Free",
    price: { kind: "free" },
    description:
      "Private, local meeting notes with on-device models or your own API keys.",
    features: [
      { label: "Unlimited on-device transcription", included: true },
      { label: "Local recordings and audio player", included: true },
      { label: "Bring your own keys for STT and AI", included: true },
      { label: "Notes, folders, and templates", included: true },
      { label: "Chat and exports", included: true },
      {
        label: "Local API, CLI, MCP, and webhooks",
        included: true,
      },
      { label: "Manual Speaker Labeling", included: true },
    ],
  },
  {
    id: "pro",
    name: "Pro",
    price: {
      kind: "fixed",
      monthly: 15,
      yearly: 150,
    },
    description:
      "Hosted transcription, AI, sync, and personal workflows for one person.",
    popular: true,
    features: [
      { label: "Everything in Free", included: true },
      { label: "Cloud Transcription", included: true },
      { label: "Cloud LLM", included: true },
      { label: "Better Speaker Identification", included: true },
      { label: "End-to-end encrypted sync across 3 devices", included: true },
      {
        label: "Share individual notes",
        included: true,
        tooltip:
          "Share with specific people or create a revocable public link.",
      },
      {
        label: "Integrations and personal automations",
        included: true,
        tooltip:
          "Run follow-up work automatically when a meeting ends — post a recap, update a page, or create issues.",
      },
      {
        label: "Folder sharing with access controls",
        included: true,
      },
      { label: "Custom dictionaries and summary formats", included: true },
    ],
  },
  {
    id: "team",
    name: "Team",
    price: {
      kind: "fixed",
      monthly: 20,
      yearly: 200,
      billingUnit: "person",
    },
    description:
      "A paid shared workspace with Pro for every member; each workspace has its own per-seat billing.",
    features: [
      { label: "Everything in Pro for every member", included: true },
      { label: "Sync across 5 devices per member", included: true },
      { label: "Shared workspaces and notes", included: true },
      { label: "Members, roles, and invitations", included: true },
      { label: "Centralized per-seat billing", included: true },
      {
        label: "Shared team folders",
        included: true,
      },
      {
        label: "Shared team templates",
        included: true,
      },
      {
        label: "Shared team automations",
        included: true,
      },
    ],
  },
  {
    id: "enterprise",
    name: "Enterprise",
    price: { kind: "custom" },
    description:
      "Organization-wide security, policy, and deployment controls with a founder-led rollout.",
    features: [
      { label: "Everything in Team", included: true },
      { label: "Domain SSO and SCIM", included: true },
      { label: "Sharing, retention, and consent policies", included: true },
      { label: "Usage and audit visibility", included: true },
      { label: "Custom workspace subdomain", included: true },
      { label: "Customer-hosted capture and data plane", included: true },
      { label: "Founder-led security review and rollout", included: true },
    ],
  },
];

export const PLAN_TIERS: PlanTierData[] = MARKETING_PLAN_TIERS.map((plan) => {
  if (plan.price.kind === "fixed") {
    const billingUnit = plan.price.billingUnit === "person" ? "/person" : "";
    return {
      id: plan.id,
      name: plan.name,
      price: `$${plan.price.monthly}`,
      period: `${billingUnit}/month`,
      subtitle: plan.price.yearly
        ? `or $${plan.price.yearly}${billingUnit}/year`
        : null,
      description: plan.description,
      features: plan.features.filter((feature) => feature.included),
    };
  }

  return {
    id: plan.id,
    name: plan.name,
    price: plan.price.kind === "free" ? "$0" : "Custom",
    period: plan.price.kind === "free" ? "/month" : "",
    subtitle: plan.price.kind === "custom" ? "Founder-led rollout" : null,
    description: plan.description,
    features: plan.features.filter((feature) => feature.included),
  };
});

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
    return { kind: "current" };
  }

  if (currentPlan === "free") {
    if (tierId === "pro" && canStartTrial) {
      return { kind: "startTrial", plan: "pro" };
    }
    return { kind: "checkout", plan: "pro", direction: "upgrade" };
  }

  if (tierId === "free") {
    return null;
  }

  return {
    kind: "checkout",
    plan: tierId,
    direction:
      TIER_ORDER[tierId] > TIER_ORDER[currentPlan] ? "upgrade" : "downgrade",
  };
}
