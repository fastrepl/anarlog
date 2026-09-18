import type Stripe from "stripe";

import { getCustomerOwner, isAutumnManagedCustomer } from "./customer-metadata";

type Product = "anarlog" | "char";

// Stripe retries deliveries that stall, so a slow Slack call must not hold the
// webhook response open.
const SLACK_TIMEOUT_MS = 3_000;

// A subscription that moves from one of these to `active` has just started paying.
const UNPAID_STATUSES: ReadonlySet<string> = new Set([
  "trialing",
  "incomplete",
]);

export type NewCustomerAlertDependencies = {
  anarlogWebhookUrl: string | undefined;
  charWebhookUrl: string | undefined;
  getCustomer: (customerId: string) => Promise<Stripe.Customer | null>;
  getProductName: (productId: string) => Promise<string>;
  postSlackMessage: (webhookUrl: string, text: string) => Promise<void>;
};

// BlackMushi and Char share one Stripe account, so Stripe's own Slack app cannot
// pick a channel; Char's customers come from Autumn and carry `autumn_id`.
// This announces BlackMushi signups (BlackMushi creates the Stripe customer at signup)
// and each plan's first paid start for both products. Char signups come from the
// Char API, because Char creates no Stripe customer when someone signs up.
export async function sendNewCustomerAlert(
  event: Stripe.Event,
  dependencies?: NewCustomerAlertDependencies,
) {
  if (event.type === "customer.created") {
    const customer = event.data.object as Stripe.Customer;
    if (
      isAutumnManagedCustomer(customer.metadata) ||
      getCustomerOwner(customer.metadata)?.kind !== "user"
    ) {
      return null;
    }
    const activeDependencies =
      dependencies ?? (await createDefaultDependencies());
    if (!activeDependencies.anarlogWebhookUrl) {
      return null;
    }
    await activeDependencies.postSlackMessage(
      activeDependencies.anarlogWebhookUrl,
      `${customerLink(event, customer)} signed up to BlackMushi`,
    );
    return { product: "anarlog" as Product, kind: "signup" as const };
  }

  if (!startsPaidPlan(event)) {
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

  const plan = escapeSlackText(
    planLabel(await activeDependencies.getProductName(productId)),
  );
  await activeDependencies.postSlackMessage(
    webhookUrl,
    `${customerLink(event, customer)} started ${plan} plan`,
  );

  return { product, kind: "plan" as const };
}

function startsPaidPlan(event: Stripe.Event) {
  if (event.type === "customer.subscription.created") {
    return (event.data.object as Stripe.Subscription).status === "active";
  }
  if (event.type === "customer.subscription.updated") {
    const previousStatus = (
      event.data.previous_attributes as { status?: string } | undefined
    )?.status;
    return (
      (event.data.object as Stripe.Subscription).status === "active" &&
      previousStatus !== undefined &&
      UNPAID_STATUSES.has(previousStatus)
    );
  }
  return false;
}

function getProductId(subscription: Stripe.Subscription) {
  const product = subscription.items?.data[0]?.price.product;
  if (!product) {
    return null;
  }
  return typeof product === "string" ? product : product.id;
}

function customerLink(event: Stripe.Event, customer: Stripe.Customer) {
  const dashboardUrl = `https://dashboard.stripe.com/${event.livemode ? "" : "test/"}customers/${customer.id}`;
  return `<${dashboardUrl}|${escapeSlackText(customer.email ?? customer.id)}>`;
}

// "BlackMushi Pro" and "Char Max" read as "Pro" and "Max" in their own channels.
function planLabel(productName: string) {
  return productName.replace(/^(BlackMushi|Char)\s+/, "");
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
