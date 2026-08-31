export type PlanTier = "free" | "pro";
export type MarketingPlanTier = PlanTier | "team";
export type PlanFeature = {
  label: string;
  included: boolean;
  tooltip?: string;
};

// Behavioral variants only: consumers render their own (translated) labels,
// so no navigation or analytics decision can depend on display copy.
export type TierAction =
  | { kind: "current" }
  | { kind: "startTrial"; plan: "pro" }
  | { kind: "checkout"; plan: "pro"; direction: "upgrade" | "downgrade" }
  | null;

export interface PlanTierData {
  id: PlanTier;
  name: string;
  price: string;
  period: string;
  subtitle: string | null;
  features: PlanFeature[];
}

export interface MarketingPlanData {
  id: MarketingPlanTier;
  name: string;
  price: { monthly: number; yearly: number | null } | null;
  billingUnit?: "person";
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
      { label: "CLI", included: true },
      { label: "MCP", included: true },
      {
        label: "Webhooks",
        included: true,
        tooltip:
          "Signed webhooks when meetings finish and summaries are generated.",
      },
      { label: "Transcript Editor", included: true },
      { label: "Shortcuts", included: true },
      { label: "Manual Speaker Labeling", included: true },
      { label: "Cloud Transcription", included: false },
      { label: "Cloud LLM", included: false },
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
      "Hosted transcription and AI for one person, plus sync, sharing, integrations, and advanced workflows.",
    popular: true,
    features: [
      { label: "Everything in Free", included: true },
      { label: "Cloud Transcription", included: true },
      { label: "Cloud LLM", included: true },
      { label: "Better Speaker Identification", included: true },
      { label: "Custom Transcription Dictionary", included: true },
      { label: "Custom Auto Summary Format", included: true },
      { label: "Custom App Icons (macOS)", included: true },
      {
        label: "Integrations",
        included: true,
        tooltip:
          "Slack, Notion, Linear, GitHub, Google Calendar, and Outlook Calendar.",
      },
      {
        label: "Automations",
        included: true,
        tooltip:
          "Run follow-up work automatically when a meeting ends — post a recap, update a page, or create issues.",
      },
      { label: "Cloud Sync", included: true },
      {
        label: "Shareable Links",
        included: true,
        tooltip: "DocSend-like: view tracking, expiration, revocation",
      },
      {
        label: "Cloud API, MCP & Webhooks",
        included: true,
        tooltip:
          "Hosted API, MCP connectors for Claude and ChatGPT, and signed webhooks — no desktop app required.",
      },
    ],
  },
  {
    id: "team",
    name: "Team",
    price: {
      monthly: 20,
      yearly: 200,
    },
    billingUnit: "person",
    description:
      "A shared workspace with Pro for every member, centralized billing, roles, and organization-wide controls.",
    features: [
      { label: "Everything in Pro for every member", included: true },
      { label: "Shared workspaces and notes", included: true },
      { label: "Members, roles, and invitations", included: true },
      { label: "Centralized per-seat billing", included: true },
      { label: "Workspace sharing policies", included: true },
      { label: "Workspace usage overview", included: true },
      { label: "Custom workspace subdomain", included: true },
    ],
  },
];

export const PLAN_TIERS: PlanTierData[] = MARKETING_PLAN_TIERS.filter(
  (plan): plan is MarketingPlanData & { id: PlanTier } =>
    plan.id === "free" || plan.id === "pro",
).map((plan) => ({
  id: plan.id,
  name: plan.name,
  price: plan.price ? `$${plan.price.monthly}` : "$0",
  period: "/month",
  subtitle: plan.price?.yearly ? `or $${plan.price.yearly}/year` : null,
  features: plan.features.filter((feature) => feature.included),
}));

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
