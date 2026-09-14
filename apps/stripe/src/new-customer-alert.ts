import type Stripe from "stripe";

import { getCustomerOwner, isAutumnManagedCustomer } from "./customer-metadata";

type Product = "anarlog" | "char";

// Stripe retries deliveries that stall, so a slow Slack call must not hold the
// webhook response open.
const SLACK_TIMEOUT_MS = 3_000;

export type NewCustomerAlertDependencies = {
  anarlogWebhookUrl: string | undefined;
  charWebhookUrl: string | undefined;
  postSlackMessage: (webhookUrl: string, text: string) => Promise<void>;
};

// Anarlog and Char share one Stripe account, so Stripe's own Slack app cannot
// pick a channel. Char's customers come from Autumn and carry `autumn_id`.
export async function sendNewCustomerAlert(
  event: Stripe.Event,
  dependencies?: NewCustomerAlertDependencies,
) {
  if (event.type !== "customer.created") {
    return null;
  }

  const customer = event.data.object as Stripe.Customer;
  const product: Product = isAutumnManagedCustomer(customer.metadata)
    ? "char"
    : "anarlog";
  const activeDependencies =
    dependencies ?? (await createDefaultDependencies());
  const webhookUrl =
    product === "char"
      ? activeDependencies.charWebhookUrl
      : activeDependencies.anarlogWebhookUrl;
  if (!webhookUrl) {
    return null;
  }

  await activeDependencies.postSlackMessage(
    webhookUrl,
    newCustomerAlertText(event, customer, product),
  );

  return { product, customerId: customer.id };
}

function newCustomerAlertText(
  event: Stripe.Event,
  customer: Stripe.Customer,
  product: Product,
) {
  const label =
    product === "char"
      ? "Char"
      : getCustomerOwner(customer.metadata)?.kind === "workspace"
        ? "Anarlog Team"
        : "Anarlog";
  const dashboardUrl = `https://dashboard.stripe.com/${event.livemode ? "" : "test/"}customers/${customer.id}`;
  const email = escapeSlackText(customer.email ?? "(no email)");
  return `New ${label} customer: ${email}\n<${dashboardUrl}|View in Stripe>`;
}

function escapeSlackText(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

async function createDefaultDependencies(): Promise<NewCustomerAlertDependencies> {
  const { env } = await import("./env");

  return {
    anarlogWebhookUrl: env.SLACK_ALERT_ANARLOG_WEBHOOK_URL,
    charWebhookUrl: env.SLACK_ALERT_CHAR_WEBHOOK_URL,
    async postSlackMessage(webhookUrl, text) {
      const response = await fetch(webhookUrl, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ text }),
        signal: AbortSignal.timeout(SLACK_TIMEOUT_MS),
      });
      if (!response.ok) {
        throw new Error(`Slack webhook responded with ${response.status}`);
      }
    },
  };
}
