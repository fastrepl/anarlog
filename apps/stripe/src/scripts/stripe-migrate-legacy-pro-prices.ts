import Stripe from "stripe";
import { parseArgs } from "util";

const HARDCODED_OLD_MONTHLY_PRICE_ID = "price_1RsWbzEABq1oJeLy4hfpEFJT";
const HARDCODED_OLD_YEARLY_PRICE_ID = "price_1RsuVFEABq1oJeLy6mPncvSp";
const HARDCODED_NEW_MONTHLY_PRICE_ID = "price_1T2Z8ZEABq1oJeLyqbCPC7cl";
const HARDCODED_NEW_YEARLY_PRICE_ID = "price_1T2Z8IEABq1oJeLyNN5InKs4";
const STRIPE_API_VERSION = "2026-01-28.clover";

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
    "new-monthly-price": {
      type: "string",
    },
    "new-yearly-price": {
      type: "string",
    },
    "old-monthly-price": {
      type: "string",
    },
    "old-yearly-price": {
      type: "string",
    },
    statuses: {
      type: "string",
      default: "active,trialing,past_due",
    },
    subscription: {
      type: "string",
    },
  },
  strict: true,
  allowPositionals: false,
});

if (values.help) {
  console.log(`
Usage: bun stripe-migrate-legacy-pro-prices.ts [options]

Migrates the legacy Char Pro prices:
  - $8/month  -> $25/month
  - $59/year  -> $250/year

The script updates matching subscription items with proration_behavior=none, so
same-interval subscriptions keep their current renewal date and pick up the new
price on the next renewal.

Options:
  --old-monthly-price <price_id>           Override the baked-in legacy $8/month price ID
  --old-yearly-price <price_id>            Override the baked-in legacy $59/year price ID
  --new-monthly-price <price_id>           Override the baked-in $25/month price ID
  --new-yearly-price <price_id>            Override the baked-in $250/year price ID
  --statuses <csv>                         Subscription statuses to include
                                           Default: active,trialing,past_due
  --limit <n>                              Only process the first n matches
  --subscription <subscription_id>         Restrict to a single subscription
  --include-cancel-at-period-end           Include subscriptions already set to cancel
  --allow-scheduled-subscriptions          Include subscriptions with schedules attached
  --allow-pending-updates                  Include subscriptions with pending updates
  --allow-multi-item-subscriptions         Include subscriptions with more than one item
  --apply                                  Apply updates. Without this, dry-run only
  -h, --help                               Show this help message

Examples:
  bun stripe-migrate-legacy-pro-prices.ts

  bun stripe-migrate-legacy-pro-prices.ts \\
    --subscription sub_123 \\
    --apply
`);
  process.exit(0);
}

const STRIPE_SECRET_KEY = Bun.env.STRIPE_SECRET_KEY;

if (!STRIPE_SECRET_KEY) {
  throw new Error("Missing required environment variable STRIPE_SECRET_KEY");
}

const oldMonthlyPriceId =
  values["old-monthly-price"] ?? HARDCODED_OLD_MONTHLY_PRICE_ID;
const oldYearlyPriceId =
  values["old-yearly-price"] ?? HARDCODED_OLD_YEARLY_PRICE_ID;
const newMonthlyPriceId =
  values["new-monthly-price"] ?? HARDCODED_NEW_MONTHLY_PRICE_ID;
const newYearlyPriceId =
  values["new-yearly-price"] ?? HARDCODED_NEW_YEARLY_PRICE_ID;

const limit = parsePositiveInteger(values.limit, "--limit");
const subscriptionFilter = values.subscription ?? null;
const shouldApply = values.apply ?? false;
const includeCancelAtPeriodEnd =
  values["include-cancel-at-period-end"] ?? false;
const allowScheduledSubscriptions =
  values["allow-scheduled-subscriptions"] ?? false;
const allowPendingUpdates = values["allow-pending-updates"] ?? false;
const allowMultiItemSubscriptions =
  values["allow-multi-item-subscriptions"] ?? false;

const allowedStatuses = parseStatuses(values.statuses);

const stripe = new Stripe(STRIPE_SECRET_KEY, {
  apiVersion: STRIPE_API_VERSION,
  maxNetworkRetries: 2,
});

const PRICE_RULES = [
  {
    key: "monthly",
    interval: "month",
    oldAmount: 800,
    newAmount: 2500,
    oldPriceId: oldMonthlyPriceId,
    newPriceId: newMonthlyPriceId,
  },
  {
    key: "yearly",
    interval: "year",
    oldAmount: 5900,
    newAmount: 25000,
    oldPriceId: oldYearlyPriceId,
    newPriceId: newYearlyPriceId,
  },
] as const;

type Rule = (typeof PRICE_RULES)[number];
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
  subscriptionId: string;
  subscriptionItemId: string;
  customerId: string | null;
  status: Stripe.Subscription.Status;
  quantity: number;
  currentPeriodEnd: number;
  cancelAtPeriodEnd: boolean;
  targetPriceId: string;
};

type RuleStats = {
  scanned: number;
  matched: number;
  skipped: Record<SkipReason, number>;
};

const livemode = await validateConfiguredPrices(PRICE_RULES);

console.log(
  `${shouldApply ? "Applying" : "Dry run"} legacy price migration in ${
    livemode ? "live" : "test"
  } mode`,
);
console.log(
  `Statuses: ${Array.from(allowedStatuses).join(", ")} | include_cancel_at_period_end=${includeCancelAtPeriodEnd} | allow_scheduled=${allowScheduledSubscriptions} | allow_pending_updates=${allowPendingUpdates} | allow_multi_item=${allowMultiItemSubscriptions}`,
);
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
    `[${rule.key}] scanned=${stats.scanned} matched=${stats.matched} skipped=${formatSkipped(stats.skipped)}`,
  );
}

console.log(`Total candidates: ${allCandidates.length}`);

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
    const updated = await stripe.subscriptionItems.update(
      candidate.subscriptionItemId,
      {
        price: candidate.targetPriceId,
        proration_behavior: "none",
        quantity: candidate.quantity,
      },
      {
        idempotencyKey: `legacy-price-migration:${candidate.subscriptionItemId}:${candidate.targetPriceId}`,
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

function parseStatuses(value: string): Set<Stripe.Subscription.Status> {
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

  const statuses = value
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
): Promise<boolean> {
  const prices = await Promise.all(
    rules.flatMap((rule) => [
      stripe.prices.retrieve(rule.oldPriceId),
      stripe.prices.retrieve(rule.newPriceId),
    ]),
  );

  const livemodeSet = new Set(prices.map((price) => price.livemode));
  if (livemodeSet.size !== 1) {
    throw new Error("Configured prices mix live and test mode Stripe objects");
  }

  const productIds = new Set(prices.map((price) => price.product));
  if (productIds.size !== 1) {
    throw new Error(
      "Configured prices do not all belong to the same Stripe product",
    );
  }

  for (const rule of rules) {
    const oldPrice = prices.find((price) => price.id === rule.oldPriceId);
    const newPrice = prices.find((price) => price.id === rule.newPriceId);

    if (!oldPrice || !newPrice) {
      throw new Error(`Failed to load prices for ${rule.key}`);
    }

    assertPrice(oldPrice, {
      amount: rule.oldAmount,
      interval: rule.interval,
      label: `${rule.key} old price`,
      requireActive: false,
    });
    assertPrice(newPrice, {
      amount: rule.newAmount,
      interval: rule.interval,
      label: `${rule.key} new price`,
      requireActive: true,
    });

    console.log(
      `[${rule.key}] ${formatPrice(oldPrice)} -> ${formatPrice(newPrice)}`,
    );
  }

  return prices[0]?.livemode ?? false;
}

function assertPrice(
  price: Stripe.Price,
  expected: {
    amount: number;
    interval: "month" | "year";
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
  remainingLimit: number | null,
): Promise<{ candidates: Candidate[]; stats: RuleStats }> {
  const candidates: Candidate[] = [];
  const stats = emptyStats();

  for await (const subscription of stripe.subscriptions.list({
    limit: 100,
    price: rule.oldPriceId,
    status: "all",
  })) {
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

    if (!allowMultiItemSubscriptions && subscription.items.data.length !== 1) {
      stats.skipped.multi_item_subscription++;
      continue;
    }

    const item = matchingItems[0];
    candidates.push({
      ruleKey: rule.key,
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
      targetPriceId: rule.newPriceId,
    });
    stats.matched++;

    if (remainingLimit !== null && candidates.length >= remainingLimit) {
      break;
    }
  }

  return { candidates, stats };
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

  return `$${(amount / 100).toFixed(2)}`;
}

function unixToIso(timestamp: number): string {
  return new Date(timestamp * 1000).toISOString();
}
