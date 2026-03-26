import * as Sentry from "@sentry/bun";
import { Hono } from "hono";

import { syncBillingBridge } from "../billing-bridge";
import { env } from "../env";
import type { AppBindings } from "../hono-bindings";
import { stripeSync } from "../integration/stripe-sync";

export const webhook = new Hono<AppBindings>();

webhook.post("/stripe", async (c) => {
  const stripeEvent = c.get("stripeEvent");
  const rawBody = c.get("stripeRawBody");
  const signature = c.get("stripeSignature");

  try {
    await stripeSync.processWebhook(rawBody, signature);
  } catch (error) {
    if (env.NODE_ENV !== "production") {
      console.error(error);
    } else {
      if (
        error instanceof Error &&
        error.message === "Unhandled webhook event"
      ) {
        Sentry.captureMessage(
          `Unhandled Stripe webhook event: ${stripeEvent.type}`,
          {
            level: "warning",
            tags: {
              webhook: "stripe",
              event_type: stripeEvent.type,
            },
            extra: {
              api_version: stripeEvent.api_version,
            },
          },
        );
      } else {
        Sentry.captureException(error, {
          tags: {
            webhook: "stripe",
            event_type: stripeEvent.type,
          },
          extra: {
            api_version: stripeEvent.api_version,
          },
        });
        return c.json({ error: "stripe_sync_failed" }, 500);
      }
    }
  }

  try {
    await syncBillingBridge(stripeEvent);
  } catch (error) {
    Sentry.captureException(error, {
      tags: { webhook: "stripe", event_type: stripeEvent.type },
    });
    return c.json({ error: "billing_bridge_sync_failed" }, 500);
  }

  return c.json({ ok: true }, 200);
});
