// Drop archived prices from a customer portal configuration's
// features.subscription_update.products list.
//
// Stripe validates the whole products list on every save, so a single archived
// price makes the configuration permanently unsaveable:
//   "Only active, per unit licensed prices are supported."
// Collapsing pricing to a single Pro plan archived the old prices without
// pruning them here, which is how that state was reached.
//
// https://docs.stripe.com/api/customer_portal/configuration
import Stripe from "stripe";
import { parseArgs } from "util";

const STRIPE_API_VERSION = "2026-02-25.clover";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    configuration: {
      type: "string",
    },
    "dry-run": {
      type: "boolean",
      default: false,
    },
  },
  strict: true,
  allowPositionals: false,
});

const dryRun = values["dry-run"] ?? false;

const { STRIPE_SECRET_KEY } = Bun.env;

if (!STRIPE_SECRET_KEY) {
  throw new Error("Missing required STRIPE_SECRET_KEY environment variable");
}

const stripe = new Stripe(STRIPE_SECRET_KEY, {
  apiVersion: STRIPE_API_VERSION,
});

const resolveConfigurationId = async () => {
  if (values.configuration) {
    return values.configuration;
  }

  const configurations = await stripe.billingPortal.configurations.list({
    is_default: true,
    limit: 1,
  });
  const defaultConfiguration = configurations.data[0];

  if (!defaultConfiguration) {
    throw new Error(
      "No default portal configuration found; pass --configuration bpc_...",
    );
  }

  return defaultConfiguration.id;
};

// Only active, per-unit licensed prices are accepted by the portal.
const isSupportedPrice = (price: Stripe.Price) =>
  price.active &&
  price.billing_scheme === "per_unit" &&
  price.recurring?.usage_type === "licensed";

const main = async () => {
  const configurationId = await resolveConfigurationId();
  const configuration =
    await stripe.billingPortal.configurations.retrieve(configurationId);

  const products = configuration.features.subscription_update.products;

  if (!products || products.length === 0) {
    console.log(`${configurationId}: no subscription_update products to prune`);
    return;
  }

  const dropped: string[] = [];
  const pruned: Stripe.BillingPortal.ConfigurationUpdateParams.Features.SubscriptionUpdate.Product[] =
    [];

  for (const product of products) {
    const prices: string[] = [];

    for (const priceId of product.prices) {
      const price = await stripe.prices.retrieve(priceId);
      if (isSupportedPrice(price)) {
        prices.push(priceId);
      } else {
        dropped.push(priceId);
      }
    }

    if (prices.length > 0) {
      pruned.push({ product: product.product, prices });
    }
  }

  if (dropped.length === 0) {
    console.log(`${configurationId}: all subscription_update prices are valid`);
    return;
  }

  console.log(`${configurationId}: dropping ${dropped.length} price(s)`);
  for (const priceId of dropped) {
    console.log(`  - ${priceId}`);
  }
  console.log(
    `  remaining: ${pruned.flatMap((product) => product.prices).join(", ") || "(none)"}`,
  );

  if (pruned.length === 0) {
    throw new Error(
      "Pruning would leave no valid prices; portal plan switching would break",
    );
  }

  if (dryRun) {
    console.log("Dry run; no changes applied");
    return;
  }

  await stripe.billingPortal.configurations.update(configurationId, {
    features: {
      subscription_update: {
        enabled: configuration.features.subscription_update.enabled,
        products: pruned,
      },
    },
  });

  console.log(`${configurationId}: updated`);
};

await main();
