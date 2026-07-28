import { Hono } from "hono";

import { captureBillingEvent, captureTrialEndingEmailSent } from "../analytics";
import { syncBillingBridge } from "../billing-bridge";
import { env } from "../env";
import { captureOperationalError } from "../error-reporting";
import type { AppBindings } from "../hono-bindings";
import { stripeSync } from "../integration/stripe-sync";
import { sendTrialEndingEmail } from "../trial-emails";

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
        captureOperationalError(error, {
          operation: "stripe_webhook_unhandled_event",
          level: "warning",
          tags: {
            webhook: "stripe",
            event_type: stripeEvent.type,
            api_version: stripeEvent.api_version ?? "unknown",
          },
        });
      } else {
        captureOperationalError(error, {
          operation: "stripe_webhook_process",
          tags: {
            event_type: stripeEvent.type,
          },
          context: {
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
    captureOperationalError(error, {
      operation: "billing_bridge_sync",
      tags: { event_type: stripeEvent.type },
    });
    return c.json({ error: "billing_bridge_sync_failed" }, 500);
  }

  try {
    await captureBillingEvent(stripeEvent);
  } catch (error) {
    captureOperationalError(error, {
      operation: "billing_analytics_capture",
      level: "warning",
      tags: {
        event_type: stripeEvent.type,
      },
    });
  }

  try {
    const receipt = await sendTrialEndingEmail(stripeEvent);
    if (receipt) {
      try {
        await captureTrialEndingEmailSent({
          eventCreated: stripeEvent.created,
          eventId: stripeEvent.id,
          ...receipt,
        });
      } catch (error) {
        captureOperationalError(error, {
          operation: "trial_email_analytics_capture",
          level: "warning",
          tags: {
            event_type: stripeEvent.type,
          },
        });
      }
    }
  } catch (error) {
    captureOperationalError(error, {
      operation: "trial_email_send",
      tags: {
        event_type: stripeEvent.type,
      },
    });
  }

  return c.json({ ok: true }, 200);
});
