import * as Sentry from "@sentry/bun";
import { createMiddleware } from "hono/factory";
import Stripe from "stripe";

import { env } from "../env";
import { stripe } from "../integration/stripe";

const cryptoProvider = Stripe.createSubtleCryptoProvider();

export const verifyStripeWebhook = createMiddleware<{
  Variables: {
    stripeEvent: Stripe.Event;
    stripeRawBody: string;
    stripeSignature: string;
  };
}>(async (c, next) => {
  const signature = c.req.header("Stripe-Signature");

  if (!signature) {
    return c.text("missing_stripe_signature", 400);
  }

  const body = await c.req.text();
  try {
    const event = await stripe.webhooks.constructEventAsync(
      body,
      signature,
      env.STRIPE_WEBHOOK_SECRET,
      undefined,
      cryptoProvider,
    );

    c.set("stripeEvent", event);
    c.set("stripeRawBody", body);
    c.set("stripeSignature", signature);
    await next();
  } catch (err) {
    Sentry.captureException(err, {
      tags: { webhook: "stripe", step: "signature_verification" },
    });
    const message = err instanceof Error ? err.message : "unknown_error";
    return c.text(message, 400);
  }
});
