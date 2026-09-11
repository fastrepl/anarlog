import type { SupabaseClient } from "@supabase/supabase-js";

export type WorkspacePlan = "team" | "enterprise";

export const accountWorkspacePlanQueryKey = ["account-workspace-plan"] as const;

export function getSubscriptionAccessEnd(subscription: {
  cancel_at?: number | null;
  current_period_end?: number | null;
  items?: { data?: Array<{ current_period_end?: number | null }> };
}): number | null {
  if (typeof subscription.cancel_at === "number") {
    return subscription.cancel_at;
  }

  const itemPeriodEnds = (subscription.items?.data ?? [])
    .map((item) => item.current_period_end)
    .filter((value): value is number => typeof value === "number");

  if (itemPeriodEnds.length > 0) {
    return Math.max(...itemPeriodEnds);
  }

  return typeof subscription.current_period_end === "number"
    ? subscription.current_period_end
    : null;
}

export function formatAccountPlanDate(date: Date) {
  return date.toLocaleDateString("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

export async function fetchWorkspacePlan({
  client,
  accessToken,
  signal,
}: {
  client: SupabaseClient;
  accessToken: string;
  signal: AbortSignal;
}): Promise<WorkspacePlan | null> {
  const authorization = `Bearer ${accessToken}`;
  const workspaces = await client
    .from("workspaces")
    .select("id")
    .eq("kind", "shared")
    .setHeader("Authorization", authorization)
    .abortSignal(signal);
  if (workspaces.error) throw workspaces.error;
  if (!Array.isArray(workspaces.data)) {
    throw new Error("Could not verify your plan. Try refreshing it.");
  }

  const tiers = await Promise.all(
    workspaces.data.map(async (workspace) => {
      if (typeof workspace.id !== "string") {
        throw new Error("Could not verify your plan. Try refreshing it.");
      }
      const access = await client
        .rpc("get_workspace_access", { p_workspace_id: workspace.id })
        .setHeader("Authorization", authorization)
        .abortSignal(signal);
      if (access.error) throw access.error;
      const row = Array.isArray(access.data) ? access.data[0] : access.data;
      const tier =
        row && typeof row === "object" && "workspace_tier" in row
          ? row.workspace_tier
          : undefined;
      if (tier !== "free" && tier !== "team" && tier !== "enterprise") {
        throw new Error("Could not verify your plan. Try refreshing it.");
      }
      return tier;
    }),
  );
  return tiers.includes("enterprise")
    ? "enterprise"
    : tiers.includes("team")
      ? "team"
      : null;
}

export function getAccountPlanCopy({
  isTrialing,
  isPaused = false,
  isPaid,
  trialDaysRemaining,
  trialEnd,
  cancelAtPeriodEnd,
  currentPeriodEnd,
  hasYcPerk = false,
  workspacePlan = null,
}: {
  isTrialing: boolean;
  isPaused?: boolean;
  isPaid: boolean;
  trialDaysRemaining: number | null;
  trialEnd: Date | null;
  cancelAtPeriodEnd: boolean;
  currentPeriodEnd: Date | null;
  hasYcPerk?: boolean;
  workspacePlan?: WorkspacePlan | null;
}): { planLabel: string; planDetail: string } {
  if (workspacePlan === "enterprise") {
    return {
      planLabel: "Enterprise",
      planDetail: "Organization-wide Team with security and policy controls.",
    };
  }

  if (workspacePlan === "team") {
    return {
      planLabel: "Team",
      planDetail: "Shared workspace with Pro for every member.",
    };
  }

  const planLabel = isTrialing ? "Pro trial" : isPaid ? "Pro" : "Free";

  if (isTrialing) {
    return {
      planLabel,
      planDetail: trialEnd
        ? `${trialDaysRemaining} ${
            trialDaysRemaining === 1 ? "day" : "days"
          } left · ends ${trialEnd.toLocaleDateString("en-US", {
            month: "long",
            day: "numeric",
          })}.`
        : "Your trial is running.",
    };
  }

  if (isPaused) {
    return {
      planLabel,
      planDetail: "Your Pro trial ended. Resume it to reactivate Pro.",
    };
  }

  if (!isPaid) {
    return {
      planLabel,
      planDetail: "On-device basics, free forever.",
    };
  }

  if (cancelAtPeriodEnd) {
    return {
      planLabel,
      planDetail: currentPeriodEnd
        ? `Cancels ${formatAccountPlanDate(currentPeriodEnd)}.`
        : "Cancels at the end of the billing period.",
    };
  }

  if (hasYcPerk) {
    return {
      planLabel,
      planDetail: "YC founder year is applied to personal Pro.",
    };
  }

  return {
    planLabel,
    planDetail: "Thanks for supporting Anarlog.",
  };
}
