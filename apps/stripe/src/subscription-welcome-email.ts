import type Stripe from "stripe";

const SUBSCRIPTION_WELCOME_TRANSACTIONAL_ID = "cmsq3t8ns0ffi0jydc6uzj1rt";

type SendTransactional = (args: {
  apiKey: string;
  transactionalId: string;
  email: string;
  dataVariables: Record<string, string | number>;
  idempotencyKey: string;
}) => Promise<void>;

export type SubscriptionWelcomeEmailDependencies = {
  apiKey: string | undefined;
  getCustomer: (customerId: string) => Promise<Stripe.Customer | null>;
  hasEarlierPaidSubscriptionInvoice: (
    customerId: string,
    invoice: Stripe.Invoice,
  ) => Promise<boolean>;
  sendTransactional: SendTransactional;
};

export async function sendSubscriptionWelcomeEmail(
  event: Stripe.Event,
  dependencies?: SubscriptionWelcomeEmailDependencies,
) {
  if (event.type !== "invoice.paid") {
    return null;
  }

  const invoice = event.data.object as Stripe.Invoice;
  if (invoice.amount_paid <= 0 || !isSubscriptionInvoice(invoice)) {
    return null;
  }

  const customerId = getCustomerId(invoice);
  if (!customerId) {
    return null;
  }

  const activeDependencies =
    dependencies ?? (await createDefaultDependencies());
  if (!activeDependencies.apiKey) {
    return null;
  }

  if (
    await activeDependencies.hasEarlierPaidSubscriptionInvoice(
      customerId,
      invoice,
    )
  ) {
    return null;
  }

  const customer = await activeDependencies.getCustomer(customerId);
  if (!customer?.email) {
    return null;
  }

  await activeDependencies.sendTransactional({
    apiKey: activeDependencies.apiKey,
    transactionalId: SUBSCRIPTION_WELCOME_TRANSACTIONAL_ID,
    email: customer.email,
    dataVariables: {
      firstName: customer.name?.trim().split(/\s+/)[0] || "there",
    },
    idempotencyKey: event.id,
  });

  return {
    invoiceId: invoice.id,
    transactionalId: SUBSCRIPTION_WELCOME_TRANSACTIONAL_ID,
  };
}

async function createDefaultDependencies(): Promise<SubscriptionWelcomeEmailDependencies> {
  const [billingBridge, stripeIntegration, loops, environment] =
    await Promise.all([
      import("./billing-bridge"),
      import("./integration/stripe"),
      import("./loops"),
      import("./env"),
    ]);

  return {
    apiKey: environment.env.LOOPS_API_KEY,
    getCustomer: billingBridge.getStripeCustomer,
    async hasEarlierPaidSubscriptionInvoice(customerId, invoice) {
      for await (const candidate of stripeIntegration.stripe.invoices.list({
        customer: customerId,
        status: "paid",
        created: { lte: invoice.created },
        limit: 100,
      })) {
        if (
          candidate.id !== invoice.id &&
          candidate.amount_paid > 0 &&
          isSubscriptionInvoice(candidate)
        ) {
          return true;
        }
      }
      return false;
    },
    sendTransactional: loops.sendLoopsTransactional,
  };
}

function getCustomerId(invoice: Stripe.Invoice) {
  if (typeof invoice.customer === "string") {
    return invoice.customer;
  }
  return invoice.customer?.id ?? null;
}

function isSubscriptionInvoice(invoice: Stripe.Invoice) {
  const legacySubscription = (
    invoice as Stripe.Invoice & {
      subscription?: string | Stripe.Subscription | null;
    }
  ).subscription;

  return (
    invoice.parent?.subscription_details != null ||
    legacySubscription != null ||
    invoice.billing_reason?.startsWith("subscription") === true
  );
}
