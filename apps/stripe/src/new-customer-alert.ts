import type Stripe from "stripe";

import { isAutumnManagedCustomer } from "./customer-metadata";

type Product = "anarlog" | "char";

// Stripe retries deliveries that stall, so a slow Slack call must not hold the
// webhook response open.
const SLACK_TIMEOUT_MS = 3_000;

export type NewCustomerAlertDependencies = {
  anarlogWebhookUrl: string | undefined;
  charWebhookUrl: string | undefined;
  getCustomer: (customerId: string) => Promise<Stripe.Customer | null>;
  getProductName: (productId: string) => Promise<string>;
  postSlackMessage: (webhookUrl: string, text: string) => Promise<void>;
};

// Anarlog and Char share one Stripe account, so Stripe's own Slack app cannot
// pick a channel. Char's customers come from Autumn and carry `autumn_id`.
// Char's card-free Max trial never creates a Stripe subscription, so the Char
// API announces trial starts itself; this covers Anarlog trials and paid
// subscriptions for both products.
export async function sendNewCustomerAlert(
  event: Stripe.Event,
  dependencies?: NewCustomerAlertDependencies,
) {
  if (event.type !== "customer.subscription.created") {
    return null;
  }

  const subscription = event.data.object as Stripe.Subscription;
  const productId = getProductId(subscription);
  if (!productId) {
    return null;
  }

  const activeDependencies =
    dependencies ?? (await createDefaultDependencies());
  const customer = await activeDependencies.getCustomer(
    typeof subscription.customer === "string"
      ? subscription.customer
      : subscription.customer.id,
  );
  if (!customer) {
    return null;
  }

  const product: Product = isAutumnManagedCustomer(customer.metadata)
    ? "char"
    : "anarlog";
  const webhookUrl =
    product === "char"
      ? activeDependencies.charWebhookUrl
      : activeDependencies.anarlogWebhookUrl;
  if (!webhookUrl) {
    return null;
  }

  const plan = planLabel(await activeDependencies.getProductName(productId));
  const action =
    subscription.status === "trialing"
      ? `started ${plan} trial`
      : `subscribed to ${plan}`;
  const dashboardUrl = `https://dashboard.stripe.com/${event.livemode ? "" : "test/"}customers/${customer.id}`;
  const who = escapeSlackText(customer.email ?? customer.id);

  await activeDependencies.postSlackMessage(
    webhookUrl,
    `<${dashboardUrl}|${who}> ${action}`,
  );

  return { product, subscriptionId: subscription.id };
}

function getProductId(subscription: Stripe.Subscription) {
  const product = subscription.items?.data[0]?.price.product;
  if (!product) {
    return null;
  }
  return typeof product === "string" ? product : product.id;
}

// "Anarlog Pro" and "Char Max" read as "Pro" and "Max" in their own channels.
function planLabel(productName: string) {
  return productName.replace(/^(Anarlog|Char)\s+/, "");
}

function escapeSlackText(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

async function createDefaultDependencies(): Promise<NewCustomerAlertDependencies> {
  const [{ env }, billingBridge, stripeIntegration] = await Promise.all([
    import("./env"),
    import("./billing-bridge"),
    import("./integration/stripe"),
  ]);

  return {
    anarlogWebhookUrl: env.SLACK_ALERT_ANARLOG_WEBHOOK_URL,
    charWebhookUrl: env.SLACK_ALERT_CHAR_WEBHOOK_URL,
    getCustomer: billingBridge.getStripeCustomer,
    async getProductName(productId) {
      return (await stripeIntegration.stripe.products.retrieve(productId)).name;
    },
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
