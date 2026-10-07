import pg from "pg";
import type Stripe from "stripe";

import { captureReferralOutcome } from "./analytics";
import { getCustomerOwner, isAutumnManagedCustomer } from "./customer-metadata";
import { env } from "./env";
import { captureOperationalError } from "./error-reporting";
import { stripe } from "./integration/stripe";
import {
  applyPreparedReferralExtension,
  planReferralExtension,
} from "./referral-months";

const pool = new pg.Pool({
  connectionString: env.DATABASE_URL,
  max: 3,
  connectionTimeoutMillis: 10_000,
});
pool.on("error", (error) =>
  captureOperationalError(error, { operation: "referral_database" }),
);

export async function recordReferralTrial(event: Stripe.Event) {
  if (event.type !== "customer.subscription.created") return;
  const subscription = event.data.object as Stripe.Subscription;
  if (
    subscription.status !== "trialing" ||
    !subscription.trial_start ||
    !subscription.trial_end ||
    subscription.metadata.referral_extension === "true"
  )
    return;
  const config = await pool.query(
    "SELECT enabled, activated_at, price_ids FROM private.referral_program WHERE id",
  );
  const program = config.rows[0];
  if (
    !program?.activated_at ||
    subscription.items.data.length !== 1 ||
    !program.price_ids.includes(subscription.items.data[0].price.id)
  )
    return;
  const customerId =
    typeof subscription.customer === "string"
      ? subscription.customer
      : subscription.customer.id;
  const customer = await stripe.customers.retrieve(customerId);
  if (customer.deleted || isAutumnManagedCustomer(customer.metadata)) return;
  const owner = getCustomerOwner(customer.metadata);
  if (owner?.kind !== "user") return;
  await pool.query(
    `INSERT INTO private.referral_trial_starts
    (referred_user_id, subscription_id, stripe_event_id, started_at, ends_at)
    VALUES ($1,$2,$3,to_timestamp($4),to_timestamp($5)) ON CONFLICT DO NOTHING`,
    [
      owner.id,
      subscription.id,
      event.id,
      subscription.trial_start,
      subscription.trial_end,
    ],
  );
}

async function processReward() {
  const client = await pool.connect();
  let lockedUser: string | undefined;
  let referralId: string | undefined;
  try {
    const config = (
      await client.query(
        "SELECT enabled, activated_at, price_ids FROM private.referral_program WHERE id",
      )
    ).rows[0];
    if (!config?.enabled) return false;
    await client.query(`INSERT INTO private.referral_month_rewards(referral_id)
      SELECT i.id FROM private.referral_invites i JOIN private.referral_trial_starts t ON t.referred_user_id = i.referred_user_id
      WHERE i.reward_policy = 'trial_month' AND t.started_at >= i.claimed_at - interval '5 minutes'
        AND NOT EXISTS (SELECT 1 FROM private.referral_month_rewards r WHERE r.referral_id=i.id)
      ON CONFLICT DO NOTHING`);
    await client.query(`INSERT INTO private.referral_events(event_key,kind,referrer_user_id,referred_user_id,referral_id,occurred_at,details)
      SELECT 'trial:' || i.id, 'referral_trial_started', i.referrer_user_id, i.referred_user_id, i.id, t.started_at,
        jsonb_build_object('subscription_id',t.subscription_id,'stripe_event_id',t.stripe_event_id)
      FROM private.referral_invites i JOIN private.referral_trial_starts t ON t.referred_user_id = i.referred_user_id
      WHERE i.reward_policy = 'trial_month'
        AND NOT EXISTS (SELECT 1 FROM private.referral_events e WHERE e.event_key='trial:' || i.id) ON CONFLICT DO NOTHING`);
    const candidate = (
      await client.query(`SELECT r.referral_id, i.referrer_user_id
      FROM private.referral_month_rewards r JOIN private.referral_invites i ON i.id = r.referral_id
      WHERE r.applied_at IS NULL AND r.next_attempt_at <= now()
        AND NOT EXISTS (SELECT 1 FROM private.referral_month_rewards earlier
          JOIN private.referral_invites other ON other.id = earlier.referral_id
          WHERE other.referrer_user_id = i.referrer_user_id AND earlier.applied_at IS NULL
            AND (earlier.created_at, earlier.referral_id) < (r.created_at, r.referral_id))
      ORDER BY r.next_attempt_at, r.created_at LIMIT 1`)
    ).rows[0];
    if (!candidate) return false;
    await client.query("BEGIN");
    const lock = (
      await client.query(
        "SELECT pg_try_advisory_xact_lock(hashtextextended($1,180003)) AS locked",
        [candidate.referrer_user_id],
      )
    ).rows[0];
    if (!lock.locked) return false;
    lockedUser = candidate.referrer_user_id;
    const reward = (
      await client.query(
        `SELECT r.*, p.stripe_customer_id, i.referred_user_id
      FROM private.referral_month_rewards r JOIN private.referral_invites i ON i.id = r.referral_id
      JOIN public.profiles p ON p.id = i.referrer_user_id WHERE r.referral_id = $1 AND r.applied_at IS NULL`,
        [candidate.referral_id],
      )
    ).rows[0];
    if (!reward) return true;
    referralId = reward.referral_id;
    if (!reward.stripe_customer_id)
      throw new Error("subscription_requires_review");
    const customer = await stripe.customers.retrieve(reward.stripe_customer_id);
    if (
      customer.deleted ||
      getCustomerOwner(customer.metadata)?.id !== lockedUser ||
      getCustomerOwner(customer.metadata)?.kind !== "user"
    )
      throw new Error("subscription_requires_review");
    let subscription: Stripe.Subscription;
    if (reward.subscription_id) {
      subscription = await stripe.subscriptions.retrieve(
        reward.subscription_id,
      );
    } else {
      const subscriptions = await stripe.subscriptions.list({
        customer: reward.stripe_customer_id,
        status: "all",
        limit: 100,
      });
      const eligible = subscriptions.data.filter(
        (s) => s.status === "active" || s.status === "trialing",
      );
      if (subscriptions.has_more || eligible.length !== 1)
        throw new Error("subscription_requires_review");
      subscription = eligible[0];
      const plan = planReferralExtension(
        subscription,
        config.price_ids,
        Math.floor(Date.now() / 1000),
      );
      // Commit the absolute target before contacting Stripe. A replay must not add another month.
      await client.query(
        `UPDATE private.referral_month_rewards SET subscription_id=$2, previous_end=$3,target_end=$4 WHERE referral_id=$1`,
        [referralId, subscription.id, plan.previousEnd, plan.targetEnd],
      );
      await client.query("COMMIT");
      return true;
    }
    const targetEnd = Number(reward.target_end);
    const ownerCustomer =
      typeof subscription.customer === "string"
        ? subscription.customer
        : subscription.customer.id;
    if (ownerCustomer !== reward.stripe_customer_id)
      throw new Error("subscription_requires_review");
    subscription = await applyPreparedReferralExtension({
      subscription,
      prices: config.price_ids,
      previousEnd: Number(reward.previous_end),
      targetEnd,
      referralId: referralId!,
      now: Math.floor(Date.now() / 1000),
      update: (id, params, options) =>
        stripe.subscriptions.update(id, params, options),
    });
    await client.query(
      "UPDATE private.referral_month_rewards SET applied_at=now(),last_error=NULL WHERE referral_id=$1",
      [referralId],
    );
    await client.query(
      `INSERT INTO private.referral_events(event_key,kind,referrer_user_id,referred_user_id,referral_id,details)
      VALUES ($1,'referral_reward_applied',$2,$3,$4,$5) ON CONFLICT DO NOTHING`,
      [
        `reward:${referralId}`,
        lockedUser,
        reward.referred_user_id,
        referralId,
        JSON.stringify({
          previous_end: Number(reward.previous_end),
          target_end: targetEnd,
          subscription_id: subscription.id,
          months: 1,
        }),
      ],
    );
    await client.query("COMMIT");
    return true;
  } catch (error) {
    await client.query("ROLLBACK");
    if (referralId) {
      const reason =
        error instanceof Error &&
        [
          "subscription_requires_review",
          "subscription_changed_before_reward",
          "extension_not_confirmed",
        ].includes(error.message)
          ? error.message
          : "provider_or_database_error";
      await client.query(
        `UPDATE private.referral_month_rewards SET attempts=attempts+1,last_error=$2,
        next_attempt_at=now()+make_interval(secs => LEAST(21600, 60 * power(2, LEAST(attempts,9))::integer)) WHERE referral_id=$1 AND applied_at IS NULL`,
        [referralId, reason],
      );
      await client.query(
        `INSERT INTO private.referral_events(event_key,kind,referrer_user_id,referral_id,details)
        VALUES ($1,'referral_reward_retry',$2,$3,$4) ON CONFLICT DO NOTHING`,
        [
          `retry:${referralId}:${Date.now()}`,
          lockedUser,
          referralId,
          JSON.stringify({ reason }),
        ],
      );
    }
    captureOperationalError(error, {
      operation: "referral_month_reward",
      context: { referral_id: referralId ?? null },
    });
    return false;
  } finally {
    try {
      await client.query("ROLLBACK");
      client.release();
    } catch (error) {
      client.release(
        error instanceof Error ? error : new Error("referral_unlock_failed"),
      );
    }
  }
}

export async function flushReferralAnalytics() {
  const { rows } = await pool.query(`SELECT * FROM private.referral_events
    WHERE kind IN ('referral_trial_started','referral_reward_applied')
      AND NOT (details ? 'analytics_delivered_at')
      AND COALESCE((details->>'analytics_retry_at')::timestamptz, occurred_at) <= now()
    ORDER BY occurred_at, id LIMIT 20`);
  for (const event of rows) {
    try {
      const delivered = await captureReferralOutcome({
        event: event.kind,
        referralId: event.referral_id,
        referrerUserId: event.referrer_user_id,
        referredUserId: event.referred_user_id,
        timestamp: event.occurred_at,
      });
      if (!delivered) return;
      await pool.query(
        `UPDATE private.referral_events SET details = details ||
        jsonb_build_object('analytics_delivered_at', now()) WHERE id=$1`,
        [event.id],
      );
    } catch (error) {
      await pool.query(
        `UPDATE private.referral_events SET details = details ||
        jsonb_build_object('analytics_retry_at', now()+interval '1 minute')
        WHERE id=$1 AND NOT (details ? 'analytics_delivered_at')`,
        [event.id],
      );
      captureOperationalError(error, { operation: "referral_analytics" });
    }
  }
}

export function startReferralMonthWorker() {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let running: Promise<void>;
  const run = async () => {
    try {
      for (let count = 0; count < 20 && !stopped; count++)
        if (!(await processReward())) break;
      await flushReferralAnalytics();
    } catch (error) {
      captureOperationalError(error, { operation: "referral_month_worker" });
    }
    if (!stopped)
      timer = setTimeout(() => {
        running = run();
      }, 15_000);
  };
  running = run();
  return async () => {
    stopped = true;
    clearTimeout(timer);
    await running;
    await pool.end();
  };
}
