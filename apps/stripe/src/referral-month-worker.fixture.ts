import { PGlite } from "@electric-sql/pglite";
import { mock } from "bun:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import type Stripe from "stripe";

const db = new PGlite();
await db.exec(`CREATE SCHEMA auth; CREATE SCHEMA private; CREATE SCHEMA stripe; CREATE SCHEMA extensions;
CREATE ROLE anon; CREATE ROLE authenticated; CREATE ROLE service_role BYPASSRLS; CREATE ROLE supabase_auth_admin;
CREATE TABLE auth.users(id uuid PRIMARY KEY,is_anonymous boolean DEFAULT false,created_at timestamptz DEFAULT now(),email_confirmed_at timestamptz DEFAULT now());
CREATE FUNCTION auth.uid() RETURNS uuid LANGUAGE sql AS $$ SELECT nullif(current_setting('test.uid',true),'')::uuid $$;
CREATE FUNCTION extensions.gen_random_bytes(n integer) RETURNS bytea LANGUAGE sql AS $$ SELECT decode(substr(md5(random()::text),1,n*2),'hex') $$;
CREATE TABLE public.profiles(id uuid PRIMARY KEY REFERENCES auth.users(id),stripe_customer_id text);
CREATE TABLE private.account_deletion_jobs(owner_user_id uuid);
CREATE TABLE public.workspaces(id uuid, stripe_customer_id text,kind text,deleted_at timestamptz,created_at timestamptz);
CREATE TABLE public.workspace_memberships(workspace_id uuid,user_id uuid,deleted_at timestamptz);
CREATE TABLE stripe.subscriptions(id text PRIMARY KEY,customer text,status text,metadata jsonb DEFAULT '{}',trial_end jsonb,default_payment_method text);
CREATE TABLE stripe.subscription_items(id text,subscription text,price text);
CREATE TABLE stripe.customers(id text,email text,name text,invoice_settings jsonb,default_source text);
CREATE FUNCTION public.custom_access_token_hook(event jsonb) RETURNS jsonb LANGUAGE sql AS $$ SELECT event $$;
GRANT USAGE ON SCHEMA auth,public,private TO authenticated,supabase_auth_admin;
GRANT EXECUTE ON FUNCTION auth.uid() TO authenticated;`);

for (const migration of [
  "20260812160000_referrals.sql",
  "20260911120000_referrals_workspace_eligibility.sql",
  "20261005020924_referral_reward_fourteen_dollars.sql",
  "20261005060000_referral_trial_months.sql",
  "20261005135631_referral_trial_inviter_eligibility.sql",
]) {
  await db.exec(
    readFileSync(
      new URL(`../../../supabase/migrations/${migration}`, import.meta.url),
      "utf8",
    ),
  );
}
const referrer = "00000000-0000-0000-0000-000000000001";
const friend = "00000000-0000-0000-0000-000000000002";
await db.query("INSERT INTO auth.users(id) VALUES ($1),($2)", [
  referrer,
  friend,
]);
await db.query(
  "INSERT INTO public.profiles(id,stripe_customer_id) VALUES ($1,'cus_ref'),($2,'cus_friend')",
  [referrer, friend],
);
await db.exec(
  "UPDATE private.referral_program SET price_ids=ARRAY['price_pro'],enabled=true",
);
await db.exec(
  "INSERT INTO stripe.subscriptions(id,customer,status,trial_end) VALUES ('sub_ref','cus_ref','trialing',to_jsonb(extract(epoch from now()+interval '3 days')::bigint)); INSERT INTO stripe.subscription_items VALUES ('si_ref','sub_ref','price_pro');",
);
await db.query("SELECT set_config('test.uid',$1,false)", [referrer]);
const {
  rows: [summary],
} = await db.query<{ data: { code: string } }>(
  "SELECT public.get_referral_summary() AS data",
);
await db.query("SELECT set_config('test.uid',$1,false)", [friend]);
assert.equal(
  (
    await db.query<{ ok: boolean }>("SELECT public.claim_referral($1) AS ok", [
      summary.data.code,
    ])
  ).rows[0].ok,
  true,
);

const now = Math.floor(Date.now() / 1000);
let live = {
  id: "sub_ref",
  customer: "cus_ref",
  status: "trialing",
  trial_end: now + 3 * 86400,
  metadata: {},
  items: {
    data: [
      {
        quantity: 1,
        current_period_end: now + 10 * 86400,
        price: {
          id: "price_pro",
          recurring: { interval: "month", interval_count: 1 },
        },
      },
    ],
  },
} as Stripe.Subscription;
const previousEnd = live.trial_end!;
const failed = Promise.withResolvers<void>();
const applied = Promise.withResolvers<void>();
let failPersistence = true;
const errors: unknown[] = [];
const query = async (sql: string, params?: unknown[]) => {
  if (
    sql.includes("UPDATE private.referral_month_rewards SET applied_at") &&
    failPersistence
  ) {
    failPersistence = false;
    throw new Error("injected persistence failure after Stripe success");
  }
  const result = await db.query(sql, params);
  if (sql.includes("VALUES ($1,'referral_reward_applied'")) applied.resolve();
  return result;
};
mock.module("pg", () => ({
  default: {
    Pool: class {
      on() {}
      query = query;
      async connect() {
        return { query, release() {} };
      }
      async end() {}
    },
  },
}));
const telemetry: string[] = [];
mock.module("./analytics", () => ({
  captureReferralOutcome: async ({ event }: { event: string }) => {
    telemetry.push(event);
    throw new Error("PostHog unavailable");
  },
}));
mock.module("./env", () => ({
  env: { DATABASE_URL: "isolated-in-memory-fixture" },
}));
mock.module("./error-reporting", () => ({
  captureOperationalError(error: unknown) {
    errors.push(error);
    failed.resolve();
  },
}));
mock.module("./integration/stripe", () => ({
  stripe: {
    customers: {
      retrieve: async (id: string) => ({
        id,
        metadata: { userId: id === "cus_ref" ? referrer : friend },
      }),
    },
    subscriptions: {
      list: async () => ({ data: [live], has_more: false }),
      retrieve: async () => live,
      update: async (_id: string, params: Stripe.SubscriptionUpdateParams) => {
        live = {
          ...live,
          trial_end: params.trial_end as number,
          metadata: params.metadata as Stripe.Metadata,
        };
        return live;
      },
    },
  },
}));
const { recordReferralTrial, startReferralMonthWorker } =
  await import("./referral-month-worker");
const event = {
  type: "customer.subscription.created",
  id: "evt_friend",
  data: {
    object: {
      id: "sub_friend",
      customer: "cus_friend",
      status: "trialing",
      trial_start: now,
      trial_end: now + 30 * 86400,
      metadata: {},
      items: { data: [{ price: { id: "price_pro" } }] },
    },
  },
} as Stripe.Event;
await recordReferralTrial(event);
await recordReferralTrial(event);
let stop = startReferralMonthWorker();
await failed.promise;
await stop();
let reward = (
  await db.query<{ applied_at: Date | null; attempts: number }>(
    "SELECT * FROM private.referral_month_rewards",
  )
).rows[0];
assert.equal(reward.applied_at, null);
assert.equal(reward.attempts, 1);
assert.ok(live.trial_end! > previousEnd);
const target = live.trial_end;
await db.exec(
  "UPDATE private.referral_month_rewards SET next_attempt_at=now()",
);
stop = startReferralMonthWorker();
await applied.promise;
await stop();
reward = (
  await db.query<{ applied_at: Date | null; attempts: number }>(
    "SELECT * FROM private.referral_month_rewards",
  )
).rows[0];
assert.ok(reward.applied_at);
assert.equal(live.trial_end, target);
assert.equal(live.metadata.referral_extension, "false");
assert.equal(
  (
    await db.query<{ n: number }>(
      "SELECT count(*)::int AS n FROM private.referral_events WHERE kind='referral_reward_applied'",
    )
  ).rows[0].n,
  1,
);
assert.equal(
  (
    await db.query<{ n: number }>(
      "SELECT count(*)::int AS n FROM private.referral_trial_starts",
    )
  ).rows[0].n,
  1,
);
assert.equal(errors.length, 1);
assert.deepEqual(telemetry, [
  "referral_trial_started",
  "referral_reward_applied",
]);
await db.close();
