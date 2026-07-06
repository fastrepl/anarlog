import Stripe from "stripe";
import { parseArgs } from "util";

const STRIPE_API_VERSION = "2026-02-25.clover";

const NEW_PRO_MONTHLY_PRICE_ID = "price_1TqFp7EABq1oJeLyfkW0WJll";
const NEW_PRO_YEARLY_PRICE_ID = "price_1TqFp7EABq1oJeLybI27ilbF";

const PRICE_RULES = [
  {
    key: "pro_monthly_25_to_15",
    sourceLabel: "Pro $25/month",
    interval: "month",
    oldAmount: 2500,
    newAmount: 1500,
    oldPriceId: "price_1T2Z8ZEABq1oJeLyqbCPC7cl",
    newPriceId: NEW_PRO_MONTHLY_PRICE_ID,
  },
  {
    key: "lite_product_monthly_8_to_15",
    sourceLabel: "Lite product $8/month",
    interval: "month",
    oldAmount: 800,
    newAmount: 1500,
    oldPriceId: "price_1TFMmIEABq1oJeLy1qgN1kG9",
    newPriceId: NEW_PRO_MONTHLY_PRICE_ID,
  },
  {
    key: "legacy_pro_monthly_8_to_15",
    sourceLabel: "Legacy Pro $8/month",
    interval: "month",
    oldAmount: 800,
    newAmount: 1500,
    oldPriceId: "price_1RsWbzEABq1oJeLy4hfpEFJT",
    newPriceId: NEW_PRO_MONTHLY_PRICE_ID,
  },
  {
    key: "pro_yearly_250_to_150",
    sourceLabel: "Pro $250/year",
    interval: "year",
    oldAmount: 25000,
    newAmount: 15000,
    oldPriceId: "price_1T2Z8IEABq1oJeLyNN5InKs4",
    newPriceId: NEW_PRO_YEARLY_PRICE_ID,
  },
  {
    key: "legacy_yearly_59_to_150",
    sourceLabel: "Legacy $59/year",
    interval: "year",
    oldAmount: 5900,
    newAmount: 15000,
    oldPriceId: "price_1RsuVFEABq1oJeLy6mPncvSp",
    newPriceId: NEW_PRO_YEARLY_PRICE_ID,
  },
  {
    key: "legacy_yearly_179_to_150",
    sourceLabel: "Legacy $179/year",
    interval: "year",
    oldAmount: 17900,
    newAmount: 15000,
    oldPriceId: "price_1RXyR9EABq1oJeLyFOdCx29M",
    newPriceId: NEW_PRO_YEARLY_PRICE_ID,
  },
  {
    key: "legacy_monthly_35_to_15",
    sourceLabel: "Legacy $35/month",
    interval: "month",
    oldAmount: 3500,
    newAmount: 1500,
    oldPriceId: "price_1RMxR4EABq1oJeLyOpEFuV2Q",
    newPriceId: NEW_PRO_MONTHLY_PRICE_ID,
  },
] as const;

type Rule = (typeof PRICE_RULES)[number];
type BillingInterval = Rule["interval"];
type SkipReason =
  | "status"
  | "cancel_at_period_end"
  | "scheduled"
  | "pending_update"
  | "multi_item_subscription"
  | "matching_item_count"
  | "subscription_filter";

type Candidate = {
  ruleKey: Rule["key"];
  sourceLabel: Rule["sourceLabel"];
  subscriptionId: string;
  subscriptionItemId: string;
  customerId: string | null;
  status: Stripe.Subscription.Status;
  quantity: number;
  currentPeriodEnd: number;
  cancelAtPeriodEnd: boolean;
  sourcePriceId: string;
  targetPriceId: string;
  amountDelta: number;
  interval: BillingInterval;
};

type RuleStats = {
  scanned: number;
  matched: number;
  skipped: Record<SkipReason, number>;
};

type StripeAdapter = {
  retrievePrice: (priceId: string) => Promise<Stripe.Price>;
  listSubscriptions: (input: {
    price: string;
    status: "all";
    limit: number;
    startingAfter?: string;
  }) => Promise<Stripe.ApiList<Stripe.Subscription>>;
  updateSubscriptionItem: (
    itemId: string,
    params: Stripe.SubscriptionItemUpdateParams,
    options: Stripe.RequestOptions,
  ) => Promise<Stripe.SubscriptionItem>;
};

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    apply: {
      type: "boolean",
    },
    "allow-multi-item-subscriptions": {
      type: "boolean",
    },
    "allow-pending-updates": {
      type: "boolean",
    },
    "allow-scheduled-subscriptions": {
      type: "boolean",
    },
    "exclude-cancel-at-period-end": {
      type: "boolean",
    },
    help: {
      type: "boolean",
      short: "h",
    },
    "include-cancel-at-period-end": {
      type: "boolean",
    },
    limit: {
      type: "string",
    },
    statuses: {
      type: "string",
      default: "active,trialing,past_due,unpaid,paused",
    },
    subscription: {
      type: "string",
    },
    "use-stripe-cli": {
      type: "boolean",
    },
  },
  strict: true,
  allowPositionals: false,
});

if (values.help) {
  console.log(`
Usage: bun stripe-migrate-legacy-pro-prices.ts [options]

Migrates paid Anarlog subscriptions to the single Pro price set:
  - monthly sources -> $15/month Pro
  - yearly sources  -> $150/year Pro

The script updates matching subscription items with proration_behavior=none, so
subscriptions keep their current billing cycle and pick up the new price on the
next invoice or renewal. It never adds a second subscription item.

Options:
  --statuses <csv>                         Subscription statuses to include
                                           Default: active,trialing,past_due,unpaid,paused
  --limit <n>                              Only process the first n matches
  --subscription <subscription_id>         Restrict to a single subscription
  --exclude-cancel-at-period-end           Skip subscriptions already set to cancel
  --include-cancel-at-period-end           Accepted for compatibility; included by default
  --allow-scheduled-subscriptions          Include subscriptions with schedules attached
  --allow-pending-updates                  Include subscriptions with pending updates
  --allow-multi-item-subscriptions         Include subscriptions with more than one item
  --use-stripe-cli                         Use the logged-in Stripe CLI for dry runs or apply
  --apply                                  Apply updates. Requires STRIPE_SECRET_KEY unless --use-stripe-cli is passed
  -h, --help                               Show this help message

Examples:
  bun stripe-migrate-legacy-pro-prices.ts --use-stripe-cli

  bun stripe-migrate-legacy-pro-prices.ts \\
    --subscription sub_123 \\
    --apply
`);
  process.exit(0);
}

const limit = parsePositiveInteger(values.limit, "--limit");
const subscriptionFilter = values.subscription ?? null;
const shouldApply = values.apply ?? false;
const includeCancelAtPeriodEnd =
  values["exclude-cancel-at-period-end"] === true
    ? false
    : (values["include-cancel-at-period-end"] ?? true);
const allowScheduledSubscriptions =
  values["allow-scheduled-subscriptions"] ?? false;
const allowPendingUpdates = values["allow-pending-updates"] ?? false;
const allowMultiItemSubscriptions =
  values["allow-multi-item-subscriptions"] ?? false;
const useStripeCli = values["use-stripe-cli"] ?? false;
const allowedStatuses = parseStatuses(values.statuses);

const stripe = createStripeAdapter({
  useStripeCli,
  stripeSecretKey: Bun.env.STRIPE_SECRET_KEY,
});

const livemode = await validateConfiguredPrices(PRICE_RULES, stripe);

console.log(
  `${shouldApply ? "Applying" : "Dry run"} Anarlog Pro price migration in ${
    livemode ? "live" : "test"
  } mode`,
);
console.log(
  `Statuses: ${Array.from(allowedStatuses).join(", ")} | include_cancel_at_period_end=${includeCancelAtPeriodEnd} | allow_scheduled=${allowScheduledSubscriptions} | allow_pending_updates=${allowPendingUpdates} | allow_multi_item=${allowMultiItemSubscriptions}`,
);
if (useStripeCli) {
  console.log(
    `Stripe client: logged-in Stripe CLI (${shouldApply ? "write apply" : "read-only dry run"})`,
  );
}
if (subscriptionFilter) {
  console.log(`Subscription filter: ${subscriptionFilter}`);
}
if (limit !== null) {
  console.log(`Limit: ${limit}`);
}

const allCandidates: Candidate[] = [];
const statsByRule = new Map<Rule["key"], RuleStats>();

let remainingLimit = limit;
for (const rule of PRICE_RULES) {
  const { candidates, stats } = await collectCandidatesForRule(
    rule,
    stripe,
    remainingLimit,
  );
  allCandidates.push(...candidates);
  statsByRule.set(rule.key, stats);

  if (remainingLimit !== null) {
    remainingLimit -= candidates.length;
    if (remainingLimit <= 0) {
      break;
    }
  }
}

allCandidates.sort((a, b) => {
  if (a.currentPeriodEnd !== b.currentPeriodEnd) {
    return a.currentPeriodEnd - b.currentPeriodEnd;
  }

  return a.subscriptionId.localeCompare(b.subscriptionId);
});

for (const rule of PRICE_RULES) {
  const stats = statsByRule.get(rule.key) ?? emptyStats();
  console.log(
    `[${rule.key}] ${rule.oldPriceId} -> ${rule.newPriceId} scanned=${stats.scanned} matched=${stats.matched} skipped=${formatSkipped(stats.skipped)}`,
  );
}

console.log(`Total candidates: ${allCandidates.length}`);
printCandidateSummary(allCandidates);

if (allCandidates.length > 0) {
  console.table(
    allCandidates.slice(0, 20).map((candidate) => ({
      rule: candidate.ruleKey,
      subscription: candidate.subscriptionId,
      item: candidate.subscriptionItemId,
      customer: candidate.customerId ?? "n/a",
      status: candidate.status,
      quantity: candidate.quantity,
      renews_at: unixToIso(candidate.currentPeriodEnd),
      cancel_at_period_end: candidate.cancelAtPeriodEnd,
    })),
  );
}

if (allCandidates.length > 20) {
  console.log(
    `... ${allCandidates.length - 20} additional candidates not shown`,
  );
}

if (!shouldApply) {
  console.log("Dry run complete. Re-run with --apply to perform updates.");
  process.exit(0);
}

let successCount = 0;
const failures: Array<{
  subscriptionId: string;
  itemId: string;
  error: string;
}> = [];

for (const [index, candidate] of allCandidates.entries()) {
  console.log(
    `[${index + 1}/${allCandidates.length}] ${candidate.ruleKey} ${candidate.subscriptionId} -> ${candidate.targetPriceId}`,
  );

  try {
    const updated = await stripe.updateSubscriptionItem(
      candidate.subscriptionItemId,
      {
        price: candidate.targetPriceId,
        proration_behavior: "none",
        quantity: candidate.quantity,
      },
      {
        idempotencyKey: `anarlog-pro-price-migration-v3:${candidate.subscriptionItemId}:${candidate.targetPriceId}`,
      },
    );

    if (updated.price.id !== candidate.targetPriceId) {
      throw new Error(
        `Updated item ${updated.id} but Stripe returned price ${updated.price.id}`,
      );
    }

    successCount++;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    failures.push({
      subscriptionId: candidate.subscriptionId,
      itemId: candidate.subscriptionItemId,
      error: message,
    });
    console.error(
      `Failed to update ${candidate.subscriptionId}/${candidate.subscriptionItemId}: ${message}`,
    );
  }
}

console.log(
  `Migration complete: updated=${successCount} failed=${failures.length} total=${allCandidates.length}`,
);

if (failures.length > 0) {
  console.table(failures);
  process.exit(1);
}

function createStripeAdapter({
  useStripeCli,
  stripeSecretKey,
}: {
  useStripeCli: boolean;
  stripeSecretKey: string | undefined;
}): StripeAdapter {
  if (useStripeCli) {
    return createStripeCliAdapter();
  }

  if (!stripeSecretKey) {
    throw new Error(
      "Missing STRIPE_SECRET_KEY. For read-only dry runs with an authenticated Stripe CLI, pass --use-stripe-cli.",
    );
  }

  const client = new Stripe(stripeSecretKey, {
    apiVersion: STRIPE_API_VERSION,
    maxNetworkRetries: 2,
  });

  return {
    retrievePrice: (priceId) => client.prices.retrieve(priceId),
    listSubscriptions: ({ startingAfter, ...input }) =>
      client.subscriptions.list({
        ...input,
        starting_after: startingAfter,
      }),
    updateSubscriptionItem: (itemId, params, options) =>
      client.subscriptionItems.update(itemId, params, options),
  };
}

function createStripeCliAdapter(): StripeAdapter {
  return {
    retrievePrice: async (priceId) =>
      runStripeCli([
        "prices",
        "retrieve",
        priceId,
        "--live",
      ]) as Promise<Stripe.Price>,
    listSubscriptions: async ({ price, status, limit, startingAfter }) => {
      const args = [
        "subscriptions",
        "list",
        "--price",
        price,
        "--status",
        status,
        "--limit",
        String(limit),
        "--live",
      ];

      if (startingAfter) {
        args.push("--starting-after", startingAfter);
      }

      return runStripeCli(args) as Promise<Stripe.ApiList<Stripe.Subscription>>;
    },
    updateSubscriptionItem: async (itemId, params, options) => {
      const price = params.price;
      const prorationBehavior = params.proration_behavior;
      const quantity = params.quantity;
      const idempotencyKey = options.idempotencyKey;

      if (!price) {
        throw new Error("Stripe CLI update requires params.price");
      }

      if (!prorationBehavior) {
        throw new Error("Stripe CLI update requires params.proration_behavior");
      }

      if (typeof quantity !== "number") {
        throw new Error("Stripe CLI update requires numeric params.quantity");
      }

      if (!idempotencyKey) {
        throw new Error("Stripe CLI update requires an idempotency key");
      }

      return runStripeCli([
        "subscription_items",
        "update",
        itemId,
        "-d",
        `price=${price}`,
        "-d",
        `quantity=${quantity}`,
        "-d",
        `proration_behavior=${prorationBehavior}`,
        "--idempotency",
        idempotencyKey,
        "--stripe-version",
        STRIPE_API_VERSION,
        "--live",
        "--confirm",
      ]) as Promise<Stripe.SubscriptionItem>;
    },
  };
}

async function runStripeCli(args: string[]) {
  const proc = Bun.spawn(["stripe", ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  if (exitCode !== 0) {
    throw new Error(
      `stripe ${args.join(" ")} failed with exit ${exitCode}: ${stderr || stdout}`,
    );
  }

  const parsed = JSON.parse(stdout);

  if (typeof parsed === "object" && parsed !== null && "error" in parsed) {
    const error = parsed.error as { message?: string };
    throw new Error(error.message ?? JSON.stringify(parsed.error));
  }

  return parsed;
}

function parsePositiveInteger(
  value: string | undefined,
  label: string,
): number | null {
  if (!value) {
    return null;
  }

  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer`);
  }

  return parsed;
}

function parseStatuses(
  value: string | undefined,
): Set<Stripe.Subscription.Status> {
  const knownStatuses = new Set<string>([
    "incomplete",
    "incomplete_expired",
    "trialing",
    "active",
    "past_due",
    "canceled",
    "unpaid",
    "paused",
  ]);

  const statuses = (value ?? "")
    .split(",")
    .map((status) => status.trim())
    .filter(Boolean);

  if (statuses.length === 0) {
    throw new Error("--statuses must include at least one subscription status");
  }

  for (const status of statuses) {
    if (!knownStatuses.has(status)) {
      throw new Error(`Unknown subscription status in --statuses: ${status}`);
    }
  }

  return new Set(statuses as Stripe.Subscription.Status[]);
}

async function validateConfiguredPrices(
  rules: readonly Rule[],
  stripe: StripeAdapter,
): Promise<boolean> {
  const uniquePriceIds = Array.from(
    new Set(rules.flatMap((rule) => [rule.oldPriceId, rule.newPriceId])),
  );
  const prices = await Promise.all(
    uniquePriceIds.map((priceId) => stripe.retrievePrice(priceId)),
  );
  const pricesById = new Map(prices.map((price) => [price.id, price]));

  const livemodeSet = new Set(prices.map((price) => price.livemode));
  if (livemodeSet.size !== 1) {
    throw new Error("Configured prices mix live and test mode Stripe objects");
  }

  for (const rule of rules) {
    const oldPrice = pricesById.get(rule.oldPriceId);
    const newPrice = pricesById.get(rule.newPriceId);

    if (!oldPrice || !newPrice) {
      throw new Error(`Failed to load prices for ${rule.key}`);
    }

    assertPrice(oldPrice, {
      amount: rule.oldAmount,
      interval: rule.interval,
      label: `${rule.key} source price`,
      requireActive: false,
    });
    assertPrice(newPrice, {
      amount: rule.newAmount,
      interval: rule.interval,
      label: `${rule.key} target price`,
      requireActive: true,
    });

    console.log(
      `[${rule.key}] ${formatPrice(oldPrice)} -> ${formatPrice(newPrice)} product ${getProductId(oldPrice)} -> ${getProductId(newPrice)}`,
    );
  }

  return prices[0]?.livemode ?? false;
}

function assertPrice(
  price: Stripe.Price,
  expected: {
    amount: number;
    interval: BillingInterval;
    label: string;
    requireActive: boolean;
  },
) {
  if (!price.recurring) {
    throw new Error(`${expected.label} must be a recurring Stripe price`);
  }

  if (price.currency !== "usd") {
    throw new Error(`${expected.label} must be USD`);
  }

  if (price.unit_amount !== expected.amount) {
    throw new Error(
      `${expected.label} must be ${formatUnitAmount(expected.amount)}, got ${formatUnitAmount(price.unit_amount)}`,
    );
  }

  if (price.recurring.interval !== expected.interval) {
    throw new Error(
      `${expected.label} must recur every ${expected.interval}, got ${price.recurring.interval}`,
    );
  }

  if (price.recurring.interval_count !== 1) {
    throw new Error(`${expected.label} must have interval_count=1`);
  }

  if (price.recurring.usage_type !== "licensed") {
    throw new Error(`${expected.label} must use licensed billing`);
  }

  if (expected.requireActive && !price.active) {
    throw new Error(`${expected.label} must be active`);
  }
}

async function collectCandidatesForRule(
  rule: Rule,
  stripe: StripeAdapter,
  remainingLimit: number | null,
): Promise<{ candidates: Candidate[]; stats: RuleStats }> {
  const candidates: Candidate[] = [];
  const stats = emptyStats();
  let startingAfter: string | undefined;

  while (true) {
    const subscriptions = await stripe.listSubscriptions({
      price: rule.oldPriceId,
      status: "all",
      limit: 100,
      startingAfter,
    });

    for (const subscription of subscriptions.data) {
      stats.scanned++;

      if (subscriptionFilter && subscription.id !== subscriptionFilter) {
        stats.skipped.subscription_filter++;
        continue;
      }

      if (!allowedStatuses.has(subscription.status)) {
        stats.skipped.status++;
        continue;
      }

      if (!includeCancelAtPeriodEnd && subscription.cancel_at_period_end) {
        stats.skipped.cancel_at_period_end++;
        continue;
      }

      if (!allowScheduledSubscriptions && subscription.schedule) {
        stats.skipped.scheduled++;
        continue;
      }

      if (!allowPendingUpdates && subscription.pending_update) {
        stats.skipped.pending_update++;
        continue;
      }

      const matchingItems = subscription.items.data.filter(
        (item) => item.price.id === rule.oldPriceId,
      );

      if (matchingItems.length !== 1) {
        stats.skipped.matching_item_count++;
        continue;
      }

      if (
        !allowMultiItemSubscriptions &&
        subscription.items.data.length !== 1
      ) {
        stats.skipped.multi_item_subscription++;
        continue;
      }

      const item = matchingItems[0];
      candidates.push({
        ruleKey: rule.key,
        sourceLabel: rule.sourceLabel,
        subscriptionId: subscription.id,
        subscriptionItemId: item.id,
        customerId:
          typeof subscription.customer === "string"
            ? subscription.customer
            : (subscription.customer?.id ?? null),
        status: subscription.status,
        quantity: item.quantity ?? 1,
        currentPeriodEnd: item.current_period_end,
        cancelAtPeriodEnd: subscription.cancel_at_period_end,
        sourcePriceId: rule.oldPriceId,
        targetPriceId: rule.newPriceId,
        amountDelta: rule.newAmount - rule.oldAmount,
        interval: rule.interval,
      });
      stats.matched++;

      if (remainingLimit !== null && candidates.length >= remainingLimit) {
        return { candidates, stats };
      }
    }

    if (!subscriptions.has_more) {
      return { candidates, stats };
    }

    startingAfter = subscriptions.data.at(-1)?.id;
    if (!startingAfter) {
      return { candidates, stats };
    }
  }
}

function printCandidateSummary(candidates: Candidate[]) {
  const summaries = PRICE_RULES.map((rule) => {
    const ruleCandidates = candidates.filter(
      (candidate) => candidate.ruleKey === rule.key,
    );

    return {
      rule: rule.key,
      source: rule.sourceLabel,
      sourcePrice: rule.oldPriceId,
      targetPrice: rule.newPriceId,
      candidates: ruleCandidates.length,
      statuses: formatStatusCounts(ruleCandidates),
      cancelAtPeriodEnd: ruleCandidates.filter(
        (candidate) => candidate.cancelAtPeriodEnd,
      ).length,
      annual: rule.interval === "year",
      priceIncrease: rule.newAmount > rule.oldAmount,
      amountChange: formatUnitAmount(rule.newAmount - rule.oldAmount),
    };
  });

  console.log("Candidate summary:");
  console.table(summaries);

  const priceIncreaseCandidates = candidates.filter(
    (candidate) => candidate.amountDelta > 0,
  ).length;
  const annualCandidates = candidates.filter(
    (candidate) => candidate.interval === "year",
  ).length;
  const cancelAtPeriodEndCandidates = candidates.filter(
    (candidate) => candidate.cancelAtPeriodEnd,
  ).length;

  console.log(
    `Flags: price_increase_candidates=${priceIncreaseCandidates} annual_candidates=${annualCandidates} cancel_at_period_end_candidates=${cancelAtPeriodEndCandidates}`,
  );
}

function formatStatusCounts(candidates: Candidate[]): string {
  const statusCounts = new Map<Stripe.Subscription.Status, number>();

  for (const candidate of candidates) {
    statusCounts.set(
      candidate.status,
      (statusCounts.get(candidate.status) ?? 0) + 1,
    );
  }

  return (
    Array.from(statusCounts.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([status, count]) => `${status}:${count}`)
      .join(", ") || "none"
  );
}

function emptyStats(): RuleStats {
  return {
    scanned: 0,
    matched: 0,
    skipped: {
      status: 0,
      cancel_at_period_end: 0,
      scheduled: 0,
      pending_update: 0,
      multi_item_subscription: 0,
      matching_item_count: 0,
      subscription_filter: 0,
    },
  };
}

function formatSkipped(skipped: RuleStats["skipped"]): string {
  return (
    Object.entries(skipped)
      .filter(([, count]) => count > 0)
      .map(([reason, count]) => `${reason}=${count}`)
      .join(", ") || "none"
  );
}

function formatPrice(price: Stripe.Price): string {
  return `${price.id} (${formatUnitAmount(price.unit_amount)} / ${price.recurring?.interval ?? "n/a"}, active=${price.active}, livemode=${price.livemode})`;
}

function formatUnitAmount(amount: number | null): string {
  if (amount === null) {
    return "unknown";
  }

  const sign = amount < 0 ? "-" : "";
  return `${sign}$${(Math.abs(amount) / 100).toFixed(2)}`;
}

function getProductId(price: Stripe.Price): string {
  if (typeof price.product === "string") {
    return price.product;
  }

  return price.product.id;
}

function unixToIso(timestamp: number): string {
  return new Date(timestamp * 1000).toISOString();
}
