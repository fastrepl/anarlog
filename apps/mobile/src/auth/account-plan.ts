import type { SupabaseClient } from "@supabase/supabase-js";

import type { BillingInfo } from "./billing";

export async function fetchWorkspacePlan({
  client,
  accessToken,
  signal,
}: {
  client: SupabaseClient;
  accessToken: string;
  signal: AbortSignal;
}): Promise<"team" | "enterprise" | null> {
  const authorization = `Bearer ${accessToken}`;
  const workspaces = await client
    .from("workspaces")
    .select("id")
    .eq("kind", "shared")
    .setHeader("Authorization", authorization)
    .abortSignal(signal);
  if (workspaces.error) throw workspaces.error;
  if (!Array.isArray(workspaces.data))
    throw new Error("Could not verify your plan. Try refreshing it.");

  const tiers = await Promise.all(
    workspaces.data.map(async (workspace) => {
      if (typeof workspace.id !== "string")
        throw new Error("Could not verify your plan. Try refreshing it.");
      const access = await client
        .rpc("get_workspace_access", { p_workspace_id: workspace.id })
        .setHeader("Authorization", authorization)
        .abortSignal(signal);
      if (access.error) throw access.error;
      const tier = access.data?.[0]?.workspace_tier;
      if (tier !== "free" && tier !== "team" && tier !== "enterprise")
        throw new Error("Could not verify your plan. Try refreshing it.");
      return tier;
    }),
  );
  return tiers.includes("enterprise")
    ? "enterprise"
    : tiers.includes("team")
      ? "team"
      : null;
}

export function accountPlanLabel(
  billing: BillingInfo,
  workspacePlan: "team" | "enterprise" | null,
) {
  if (workspacePlan === "enterprise") return "Anarlog Enterprise";
  if (workspacePlan === "team") return "Anarlog Pro";
  if (billing.plan === "trial")
    return `Pro trial · ${billing.trialDaysRemaining ?? 0} days left`;
  return billing.plan === "pro" ? "Anarlog Pro" : "Free";
}
