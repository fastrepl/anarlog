import { createClient } from "@supabase/supabase-js";
import assert from "node:assert/strict";
import test from "node:test";

import { accountPlanLabel, fetchWorkspacePlan } from "./account-plan.ts";
import { deriveBillingInfo } from "./billing.ts";

function fixture(workspaceTiers, failure) {
  const requests = [];
  const client = createClient(
    "https://example.supabase.co",
    "public-test-key",
    {
      auth: { persistSession: false, autoRefreshToken: false },
      global: {
        fetch: async (input, init) => {
          const url = new URL(input);
          requests.push({ url, init });
          assert.equal(
            new Headers(init.headers).get("Authorization"),
            "Bearer account-a-token",
          );
          if (failure?.path === url.pathname)
            return Response.json(
              { message: "Plan lookup failed" },
              { status: 403 },
            );
          if (url.pathname === "/rest/v1/workspaces") {
            assert.equal(url.searchParams.get("kind"), "eq.shared");
            assert.equal(url.searchParams.get("select"), "id");
            return Response.json(
              workspaceTiers.map((_, index) => ({ id: `workspace-${index}` })),
            );
          }
          assert.equal(url.pathname, "/rest/v1/rpc/get_workspace_access");
          const { p_workspace_id } = JSON.parse(init.body);
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

test("a Team member with the shared Pro entitlement is shown as Pro", async () => {
  const billing = deriveBillingInfo({
    subscription_status: "active",
    entitlements: ["hyprnote_pro"],
  });
  const { load } = fixture(["free", "team"]);
  assert.equal(accountPlanLabel(billing, await load()), "Anarlog Pro");
  assert.equal(billing.isPro, true);
});

test("Enterprise takes precedence over Team across memberships", async () => {
  for (const tiers of [
    ["team", "enterprise"],
    ["enterprise", "team"],
  ]) {
    const { load } = fixture(tiers);
    assert.equal(
      accountPlanLabel(deriveBillingInfo(null), await load()),
      "Anarlog Enterprise",
    );
  }
});

test("a free workspace does not upgrade an individual Pro subscription", async () => {
  const { load } = fixture(["free"]);
  assert.equal(
    accountPlanLabel(
      deriveBillingInfo({ entitlements: ["hyprnote_pro"] }),
      await load(),
    ),
    "Anarlog Pro",
  );
});

test("an account with no shared workspaces keeps its individual plan", async () => {
  const { load, requests } = fixture([]);
  const tier = await load();
  assert.equal(tier, null);
  assert.equal(requests.length, 1);
  assert.equal(accountPlanLabel(deriveBillingInfo(null), tier), "Free");
  const now = Date.UTC(2026, 8, 7);
  const trial = deriveBillingInfo(
    {
      subscription_status: "trialing",
      trial_end: now / 1000 + 21 * 86400,
    },
    now,
  );
  assert.equal(accountPlanLabel(trial, tier), "Pro trial · 21 days left");
  assert.equal(accountPlanLabel(trial, "team"), "Anarlog Pro");
});

test("a plan refresh reflects a workspace subscription ending", async () => {
  const tiers = ["team"];
  const { load } = fixture(tiers);
  assert.equal(await load(), "team");
  tiers[0] = "free";
  assert.equal(await load(), null);
});

test("failed lookups do not silently mislabel a Team member as Pro", async () => {
  for (const path of [
    "/rest/v1/workspaces",
    "/rest/v1/rpc/get_workspace_access",
  ]) {
    const { load } = fixture(["team"], { path });
    await assert.rejects(load(), { message: "Plan lookup failed" });
  }
});

test("an unknown workspace tier cannot silently fall back to Pro", async () => {
  const { load } = fixture(["unknown"]);
  await assert.rejects(load(), /Could not verify your plan/);
});
