import { expect, test } from "bun:test";

// Module replacements live in a subprocess so other billing tests keep their
// real integrations and the route keeps its production error policy.
test("the webhook route completes once its database recovers", async () => {
  const child = Bun.spawn(
    [
      process.execPath,
      "--eval",
      `
    import { mock } from "bun:test";
    import assert from "node:assert/strict";
    import { Hono } from "hono";
    mock.module("./src/env", () => ({ env: { NODE_ENV: "production" } }));
    const receipts = new Set();
    let recovering = true;
    mock.module("./src/integration/stripe-sync", () => ({ stripeSync: {
      async processWebhook(body, signature) {
        assert.equal(signature, "verified-signature");
        if (recovering) { recovering = false; throw { code: "57P03" }; }
        receipts.add(JSON.parse(body).id);
      },
    } }));
    const reconciled = new Set();
    mock.module("./src/billing-bridge", () => ({ async syncBillingBridge(event) {
      assert.ok(receipts.has(event.id));
      reconciled.add(event.id);
    } }));
    for (const [path, names] of [
      ["analytics", ["captureBillingEvent", "captureTrialEndingEmailSent"]],
      ["new-customer-alert", ["sendNewCustomerAlert"]],
      ["personal-plan-transition", ["scheduleReplacedPersonalPlanCancellation"]],
      ["referral-rewards", ["issueReferralReward"]],
      ["subscription-welcome-email", ["sendSubscriptionWelcomeEmail"]],
      ["trial-emails", ["sendTrialEndingEmail"]],
    ]) mock.module("./src/" + path, () => Object.fromEntries(names.map(name => [name, async () => {}])));
    mock.module("./src/error-reporting", () => ({ captureOperationalError(error) { throw error; } }));
    const { webhook } = await import("./src/routes/webhook");
    const event = { id: "evt_recovery", type: "customer.created", data: { object: { id: "cus_recovery" } } };
    const app = new Hono();
    app.use("*", async (c, next) => {
      c.set("stripeEvent", event);
      c.set("stripeRawBody", JSON.stringify(event));
      c.set("stripeSignature", "verified-signature");
      await next();
    });
    app.route("/webhook", webhook);
    const response = await app.request("/webhook/stripe", { method: "POST" });
    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), { ok: true });
    assert.deepEqual([...receipts], [event.id]);
    assert.deepEqual([...reconciled], [event.id]);
  `,
    ],
    {
      cwd: new URL("..", import.meta.url).pathname,
      stdout: "pipe",
      stderr: "pipe",
    },
  );
  const [code, stderr] = await Promise.all([
    child.exited,
    new Response(child.stderr).text(),
  ]);
  expect(stderr).toBe("");
  expect(code).toBe(0);
});
