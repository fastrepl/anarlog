import type { AppBindings } from "./hono-bindings";

Bun.env.DATABASE_URL ??= "postgres://localhost/test";
Bun.env.SUPABASE_URL ??= "http://localhost:54321";
Bun.env.SUPABASE_SERVICE_ROLE_KEY ??= "test";
Bun.env.STRIPE_SECRET_KEY ??= "sk_test_123";
Bun.env.STRIPE_WEBHOOK_SECRET ??= "whsec_test";

const { expect, it } = await import("bun:test");
const { Hono } = await import("hono");
const Stripe = (await import("stripe")).default;
const { verifyStripeWebhook } = await import("./middleware/stripe");
const { env } = await import("./env");

const payload = JSON.stringify({
  id: "evt_1",
  object: "event",
  type: "customer.created",
  data: { object: {} },
});

function webhookApp() {
  const app = new Hono<AppBindings>();
  let handlerReached = false;

  app.post("/webhook/stripe", verifyStripeWebhook, (c) => {
    handlerReached = true;
    return c.json({ type: c.get("stripeEvent").type });
  });

  return {
    app,
    handlerReached: () => handlerReached,
  };
}

it("rejects a missing Stripe signature before reaching the handler", async () => {
  const { app, handlerReached } = webhookApp();
  const response = await app.request("/webhook/stripe", {
    method: "POST",
    body: payload,
  });

  expect(response.status).toBe(400);
  expect(handlerReached()).toBe(false);
});

it("rejects a Stripe signature from a different secret", async () => {
  const { app, handlerReached } = webhookApp();
  const signature = await Stripe.webhooks.generateTestHeaderStringAsync({
    payload,
    secret: "whsec_other",
  });
  const response = await app.request("/webhook/stripe", {
    method: "POST",
    headers: { "Stripe-Signature": signature },
    body: payload,
  });

  expect(response.status).toBe(400);
  expect(handlerReached()).toBe(false);
});

it("accepts a valid Stripe signature and exposes the parsed event", async () => {
  const { app, handlerReached } = webhookApp();
  const signature = await Stripe.webhooks.generateTestHeaderStringAsync({
    payload,
    secret: env.STRIPE_WEBHOOK_SECRET,
  });
  const response = await app.request("/webhook/stripe", {
    method: "POST",
    headers: { "Stripe-Signature": signature },
    body: payload,
  });

  expect(response.status).toBe(200);
  expect(await response.json()).toEqual({ type: "customer.created" });
  expect(handlerReached()).toBe(true);
});
