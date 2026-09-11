import { createClient } from "@supabase/supabase-js";
import assert from "node:assert/strict";
import test from "node:test";

import {
  fetchWorkspacePlan,
  formatAccountPlanDate,
  getAccountPlanCopy,
  getSubscriptionAccessEnd,
} from "./account-plan.ts";

test("prefers cancel_at, then item period end, then subscription period end", () => {
  assert.equal(
    getSubscriptionAccessEnd({
      cancel_at: 100,
      current_period_end: 200,
      items: { data: [{ current_period_end: 300 }] },
    }),
    100,
  );
  assert.equal(
    getSubscriptionAccessEnd({
      cancel_at: null,
      items: {
        data: [{ current_period_end: 200 }, { current_period_end: 350 }],
      },
    }),
    350,
  );
  assert.equal(getSubscriptionAccessEnd({ current_period_end: 400 }), 400);
  assert.equal(getSubscriptionAccessEnd({}), null);
});

test("paused cardless trial copy explains that Pro can be resumed", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaused: true,
      isPaid: false,
      trialDaysRemaining: 0,
      trialEnd: new Date("2026-08-24T00:00:00.000Z"),
      cancelAtPeriodEnd: false,
      currentPeriodEnd: null,
    }),
    {
      planLabel: "Free",
      planDetail: "Your Pro trial ended. Resume it to reactivate Pro.",
    },
  );
});

test("paid copy acknowledges a scheduled cancellation", () => {
  const currentPeriodEnd = new Date("2026-09-17T00:00:00.000Z");

  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: true,
      currentPeriodEnd,
    }),
    {
      planLabel: "Pro",
      planDetail: `Cancels ${formatAccountPlanDate(currentPeriodEnd)}.`,
    },
  );

  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: true,
      currentPeriodEnd: null,
    }),
    {
      planLabel: "Pro",
      planDetail: "Cancels at the end of the billing period.",
    },
  );
});

test("paid copy names the YC founder year when that perk is on the subscription", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: false,
      currentPeriodEnd: new Date("2026-09-17T00:00:00.000Z"),
      hasYcPerk: true,
    }),
    {
      planLabel: "Pro",
      planDetail: "YC founder year is applied to personal Pro.",
    },
  );
});

test("paid copy stays supportive when the subscription is not canceling", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: false,
      currentPeriodEnd: new Date("2026-09-17T00:00:00.000Z"),
    }),
    {
      planLabel: "Pro",
      planDetail: "Thanks for supporting Anarlog.",
    },
  );
});

test("a Team member with the shared Pro entitlement is shown as Team", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: false,
      currentPeriodEnd: new Date("2026-09-17T00:00:00.000Z"),
      workspacePlan: "team",
    }),
    {
      planLabel: "Team",
      planDetail: "Shared workspace with Pro for every member.",
    },
  );
});

test("Enterprise takes precedence over personal Pro and Team copy", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: true,
      isPaid: true,
      trialDaysRemaining: 3,
      trialEnd: new Date("2026-09-17T00:00:00.000Z"),
      cancelAtPeriodEnd: false,
      currentPeriodEnd: null,
      workspacePlan: "enterprise",
    }),
    {
      planLabel: "Enterprise",
      planDetail: "Organization-wide Team with security and policy controls.",
    },
  );
});

test("a free workspace does not upgrade an individual Pro subscription", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: false,
      currentPeriodEnd: new Date("2026-09-17T00:00:00.000Z"),
      workspacePlan: null,
    }),
    {
      planLabel: "Pro",
      planDetail: "Thanks for supporting Anarlog.",
    },
  );
});

function workspacePlanFixture(
  workspaceTiers: string[],
  failure?: { path: string },
) {
  const requests: Array<{ url: URL; init?: RequestInit }> = [];
  const client = createClient(
    "https://example.supabase.co",
    "public-test-key",
    {
      auth: { persistSession: false, autoRefreshToken: false },
      global: {
        fetch: async (input, init) => {
          const url = new URL(String(input));
          requests.push({ url, init });
          assert.equal(
            new Headers(init?.headers).get("Authorization"),
            "Bearer account-a-token",
          );
          if (failure?.path === url.pathname) {
            return Response.json(
              { message: "Plan lookup failed" },
              { status: 403 },
            );
          }
          if (url.pathname === "/rest/v1/workspaces") {
            assert.equal(url.searchParams.get("kind"), "eq.shared");
            assert.equal(url.searchParams.get("select"), "id");
            return Response.json(
              workspaceTiers.map((_, index) => ({ id: `workspace-${index}` })),
            );
          }
          assert.equal(url.pathname, "/rest/v1/rpc/get_workspace_access");
          const { p_workspace_id } = JSON.parse(String(init?.body));
          const index = Number(p_workspace_id.replace("workspace-", ""));
          return Response.json([{ workspace_tier: workspaceTiers[index] }]);
        },
      },
    },
  );
  return {
    requests,
    load: () =>
      fetchWorkspacePlan({
        client,
        accessToken: "account-a-token",
        signal: new AbortController().signal,
      }),
  };
}

test("workspace plan lookup prefers Enterprise over Team", async () => {
  for (const tiers of [
    ["team", "enterprise"],
    ["enterprise", "team"],
  ]) {
    const { load } = workspacePlanFixture(tiers);
    assert.equal(await load(), "enterprise");
  }
});

test("workspace plan lookup returns Team for a paid shared workspace", async () => {
  const { load } = workspacePlanFixture(["free", "team"]);
  assert.equal(await load(), "team");
});

test("an account with no shared workspaces keeps its individual plan", async () => {
  const { load, requests } = workspacePlanFixture([]);
  assert.equal(await load(), null);
  assert.equal(requests.length, 1);
});

test("failed lookups do not silently mislabel a Team member as Pro", async () => {
  for (const path of [
    "/rest/v1/workspaces",
    "/rest/v1/rpc/get_workspace_access",
  ]) {
    const { load } = workspacePlanFixture(["team"], { path });
    await assert.rejects(load(), { message: "Plan lookup failed" });
  }
});

test("an unknown workspace tier cannot silently fall back to Pro", async () => {
  const { load } = workspacePlanFixture(["unknown"]);
  await assert.rejects(load(), /Could not verify your plan/);
});
