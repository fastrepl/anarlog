import { createClient } from "@supabase/supabase-js";
import Stripe from "stripe";
import { parseArgs } from "util";

import { getCustomerIdentityMetadata } from "../customer-metadata";

const STRIPE_API_VERSION = "2026-02-25.clover";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    apply: { type: "boolean", default: false },
    concurrency: { type: "string", default: "10" },
    limit: { type: "string" },
  },
  strict: true,
  allowPositionals: false,
});

const apply = values.apply ?? false;
const concurrency = parsePositiveInteger(values.concurrency, "concurrency");
const limit = values.limit ? parsePositiveInteger(values.limit, "limit") : null;
const { STRIPE_SECRET_KEY, SUPABASE_SERVICE_ROLE_KEY, SUPABASE_URL } = Bun.env;

if (!STRIPE_SECRET_KEY || !SUPABASE_SERVICE_ROLE_KEY || !SUPABASE_URL) {
  throw new Error(
    "Missing STRIPE_SECRET_KEY, SUPABASE_SERVICE_ROLE_KEY, or SUPABASE_URL",
  );
}

const stripe = new Stripe(STRIPE_SECRET_KEY, {
  apiVersion: STRIPE_API_VERSION,
});
const supabase = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY, {
  auth: {
    autoRefreshToken: false,
    persistSession: false,
  },
});

const profiles: { id: string; stripe_customer_id: string }[] = [];
const pageSize = 1000;

for (
  let offset = 0;
  limit === null || profiles.length < limit;
  offset += pageSize
) {
  const end =
    offset +
    Math.min(pageSize, limit === null ? pageSize : limit - profiles.length) -
    1;
  const { data, error } = await supabase
    .from("profiles")
    .select("id,stripe_customer_id")
    .not("stripe_customer_id", "is", null)
    .order("id")
    .range(offset, end);

  if (error) {
    throw error;
  }

  const rows = (data ?? []) as { id: string; stripe_customer_id: string }[];
  profiles.push(...rows);

  if (rows.length < pageSize || (limit !== null && profiles.length >= limit)) {
    break;
  }
}

const totals = {
  scanned: 0,
  current: 0,
  updates: 0,
  deleted: 0,
  missing: 0,
  conflicts: 0,
  errors: 0,
};

for (let offset = 0; offset < profiles.length; offset += concurrency) {
  await Promise.all(
    profiles.slice(offset, offset + concurrency).map(async (profile) => {
      totals.scanned++;

      try {
        const customer = await stripe.customers.retrieve(
          profile.stripe_customer_id,
        );
        if ("deleted" in customer && customer.deleted) {
          totals.deleted++;
          return;
        }

        const ownerIds = [
          customer.metadata["userId"],
          customer.metadata["user_id"],
          customer.metadata["userID"],
        ].filter((ownerId): ownerId is string => Boolean(ownerId));
        if (ownerIds.some((ownerId) => ownerId !== profile.id)) {
          totals.conflicts++;
          return;
        }

        const metadata = getCustomerIdentityMetadata(
          customer.metadata,
          profile.id,
        );
        if (!metadata) {
          totals.current++;
          return;
        }

        totals.updates++;
        if (apply) {
          await stripe.customers.update(profile.stripe_customer_id, {
            metadata,
          });
        }
      } catch (error) {
        if (
          error instanceof Stripe.errors.StripeError &&
          error.statusCode === 404
        ) {
          totals.missing++;
          return;
        }

        totals.errors++;
      }
    }),
  );
}

console.log(
  JSON.stringify({
    mode: apply ? "apply" : "dry-run",
    ...totals,
  }),
);

if (totals.conflicts > 0 || totals.errors > 0) {
  process.exitCode = 1;
}

function parsePositiveInteger(value: string, name: string) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}
