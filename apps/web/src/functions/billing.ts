import { createServerFn } from "@tanstack/react-start";
import type Stripe from "stripe";
import { z } from "zod";

import {
  canStartTrial as canStartTrialApi,
  deleteAccount as deleteAccountApi,
} from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";

import { env, requireEnv } from "@/env";
import { getRequestAppOrigin } from "@/functions/app-origin";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import {
  claimPendingReferral,
  getReferralTrialDays,
} from "@/functions/referrals";
import { getStripeClient } from "@/functions/stripe";
import {
  getSupabaseAdminClient,
  getSupabaseServerClient,
} from "@/functions/supabase";
import { getSubscriptionAccessEnd } from "@/lib/account-plan";
import {
  addInternalReturnPathSearch,
  sanitizeInternalReturnPath,
  toAbsoluteInternalReturnUrl,
} from "@/lib/auth-redirect";
import {
  checkoutSourceSchema,
  type CheckoutSource,
} from "@/lib/checkout-source";
import { captureOperationalError } from "@/lib/error-reporting";
import { captureServerAnalytics } from "@/lib/server-analytics";
import {
  getStripeCustomerIdentityMetadata,
  getStripeCustomerOwnership,
} from "@/lib/stripe-customer";
import { WEB_TRIAL_CHECKOUT_FIELDS } from "@/lib/trial-policy";
import {
  startWorkspaceCheckout,
  type WorkspaceCheckoutContext,
} from "@/lib/workspace-checkout";
import { isYcPromotionCode, normalizeYcPromotionCode } from "@/lib/yc-perk";
import {
  applyYcPromotionToCustomer,
  findYcPromotionCodeByCustomerCode,
  subscriptionHasYcPerk,
} from "@/lib/yc-perk-apply";

type SupabaseClient = ReturnType<typeof getSupabaseServerClient>;

type AuthUser = {
  id: string;
  email?: string | null;
  user_metadata?: {
    full_name?: unknown;
    name?: unknown;
  };
};

function getAuthUserName(user: AuthUser) {
  for (const value of [
    user.user_metadata?.full_name,
    user.user_metadata?.name,
  ]) {
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }

  return undefined;
}

class TrialCheckoutCreationError extends Error {
  constructor(readonly checkoutError: unknown) {
    super("Could not create trial checkout session");
  }
}

export const getStripeCustomerIdForUser = async (
  supabase: SupabaseClient,
  stripe: Stripe,
  user: AuthUser,
) => {
  const { data: profile, error: profileError } = await supabase
    .from("profiles")
    .select("stripe_customer_id")
    .eq("id", user.id)
    .single();

  if (profileError) {
    throw profileError;
  }

  const stripeCustomerId = profile?.stripe_customer_id as
    | string
    | null
    | undefined;

  if (!stripeCustomerId) {
    return null;
  }

  const customer = await stripe.customers.retrieve(stripeCustomerId);
  if ("deleted" in customer && customer.deleted) {
    throw new Error("Stripe customer is unavailable");
  }

  const ownership = getStripeCustomerOwnership(customer, user);
  if (ownership === "unowned") {
    throw new Error("Stripe customer does not belong to authenticated user");
  }

  const updates: Stripe.CustomerUpdateParams = {};
  const identityMetadata = getStripeCustomerIdentityMetadata(
    customer.metadata,
    user.id,
  );
  if (identityMetadata) {
    updates.metadata = identityMetadata;
  }

  const name = getAuthUserName(user);
  if (!customer.name && name) {
    updates.name = name;
  }

  if (Object.keys(updates).length > 0) {
    await stripe.customers.update(stripeCustomerId, updates);
  }

  return stripeCustomerId;
};

const getBillingReturnUrl = (scheme?: z.infer<typeof desktopSchemeSchema>) => {
  const appOrigin = getRequestAppOrigin();

  if (scheme) {
    return `${appOrigin}/callback/billing?scheme=${scheme}`;
  }

  return `${appOrigin}/app/account`;
};

export const portalIntentSchema = z.enum(["manage", "payment_method_update"]);

// Cardless trials cancel unless a card is added, so add-card CTAs must land on
// the card form. The portal home page leads with "Cancel subscription".
const paymentMethodUpdateFlow = (
  returnUrl: string,
): Stripe.BillingPortal.SessionCreateParams.FlowData => ({
  type: "payment_method_update",
  after_completion: {
    type: "redirect",
    redirect: { return_url: returnUrl },
  },
});

const getProPriceId = (period: "monthly" | "yearly") => {
  if (period === "yearly") {
    return requireEnv(env.STRIPE_YEARLY_PRICE_ID, "STRIPE_YEARLY_PRICE_ID");
  }

  return requireEnv(env.STRIPE_MONTHLY_PRICE_ID, "STRIPE_MONTHLY_PRICE_ID");
};

async function getCurrentSubscription(
  stripe: Stripe,
  stripeCustomerId: string,
  options?: { expandDiscounts?: boolean },
): Promise<Stripe.Subscription | null> {
  const subscriptions = await stripe.subscriptions.list({
    customer: stripeCustomerId,
    status: "all",
    limit: 10,
    ...(options?.expandDiscounts ? { expand: ["data.discounts"] } : {}),
  });

  return (
    subscriptions.data.find((sub) => sub.status === "active") ||
    subscriptions.data.find((sub) => sub.status === "trialing") ||
    null
  );
}

async function ensureStripeCustomerId(
  supabase: SupabaseClient,
  user: AuthUser & { email?: string | null },
) {
  const stripe = getStripeClient();
  const existingStripeCustomerId = await getStripeCustomerIdForUser(
    supabase,
    stripe,
    user,
  );

  if (existingStripeCustomerId) {
    return existingStripeCustomerId;
  }

  const newCustomer = await stripe.customers.create(
    {
      email: user.email ?? undefined,
      name: getAuthUserName(user),
      metadata: {
        userId: user.id,
        posthog_person_distinct_id: user.id,
      },
    },
    { idempotencyKey: `create-customer-${user.id}` },
  );

  const admin = getSupabaseAdminClient();
  const { data, error } = await admin.rpc("assign_profile_stripe_customer", {
    p_owner_user_id: user.id,
    p_stripe_customer_id: newCustomer.id,
  });
  let assignedCustomerId = data?.[0]?.assigned_customer_id as
    | string
    | null
    | undefined;
  if (error) {
    if (error.code === "PGRST202") {
      const { error: legacyAssignmentError } = await supabase
        .from("profiles")
        .update({ stripe_customer_id: newCustomer.id })
        .eq("id", user.id)
        .is("stripe_customer_id", null);
      if (legacyAssignmentError) {
        await stripe.customers.del(newCustomer.id).catch(() => undefined);
        throw legacyAssignmentError;
      }
    }
    const { data: linkedProfile, error: lookupError } = await admin
      .from("profiles")
      .select("stripe_customer_id")
      .eq("id", user.id)
      .single();
    if (lookupError) {
      await stripe.customers.del(newCustomer.id).catch(() => undefined);
      throw error;
    }
    assignedCustomerId = linkedProfile?.stripe_customer_id as
      | string
      | null
      | undefined;
    if (!assignedCustomerId) {
      await stripe.customers.del(newCustomer.id).catch(() => undefined);
      throw error;
    }
  }

  if (!assignedCustomerId) {
    await stripe.customers.del(newCustomer.id).catch(() => undefined);
    throw new Error("Billing is unavailable while account deletion is pending");
  }

  if (assignedCustomerId !== newCustomer.id) {
    await stripe.customers.del(newCustomer.id).catch(() => undefined);
  }
  const verifiedCustomerId = await getStripeCustomerIdForUser(
    supabase,
    stripe,
    user,
  );
  if (verifiedCustomerId !== assignedCustomerId) {
    throw new Error("Stripe customer assignment could not be verified");
  }

  return assignedCustomerId;
}

const ycPerkReturnSchema = z.enum(["applied", "claimed", "invalid"]);

function getAccountYcPerkUrl(
  scheme: z.infer<typeof desktopSchemeSchema> | undefined,
  perk: z.infer<typeof ycPerkReturnSchema>,
) {
  if (scheme) {
    return `${getBillingReturnUrl(scheme)}&perk=${perk}`;
  }

  return `${getRequestAppOrigin()}/app/account?perk=${perk}`;
}

async function createCheckoutUrl({
  supabase,
  user,
  period,
  scheme,
  trial = false,
  reservationId,
  trialDays,
  source = "unknown",
  returnTo,
  promotionCodeId,
}: {
  supabase: SupabaseClient;
  user: AuthUser & { email?: string | null };
  period: "monthly" | "yearly";
  scheme?: z.infer<typeof desktopSchemeSchema>;
  trial?: boolean;
  reservationId?: string;
  trialDays?: number;
  source?: CheckoutSource;
  returnTo?: string;
  promotionCodeId?: string;
}) {
  const stripe = getStripeClient();
  const stripeCustomerId = await ensureStripeCustomerId(supabase, user);

  if (trial) {
    if (!reservationId) {
      throw new Error("Trial reservation is required");
    }

    const subscriptions = await stripe.subscriptions.list({
      customer: stripeCustomerId,
      status: "all",
      limit: 1,
    });
    if (subscriptions.data.length > 0) {
      throw new Error("Trial is not available for this account");
    }
  }

  const checkoutType = trial ? "trial" : "paid";
  const appOrigin = getRequestAppOrigin();
  const successReturnPath = addInternalReturnPathSearch(returnTo, {
    success: "true",
    checkout: checkoutType,
    source,
  });
  const cancelReturnPath = addInternalReturnPathSearch(returnTo, {
    checkout: "canceled",
    checkout_type: checkoutType,
    source,
  });

  const successUrl = scheme
    ? `${getBillingReturnUrl(scheme)}&checkout=${checkoutType}&source=${source}`
    : toAbsoluteInternalReturnUrl(appOrigin, successReturnPath);
  const cancelUrl = scheme
    ? `${getBillingReturnUrl(scheme)}&checkout=canceled&checkout_type=${checkoutType}&source=${source}`
    : toAbsoluteInternalReturnUrl(appOrigin, cancelReturnPath);

  let checkout: Stripe.Checkout.Session;
  try {
    checkout = await stripe.checkout.sessions.create(
      {
        customer: stripeCustomerId,
        success_url: successUrl,
        cancel_url: cancelUrl,
        line_items: [
          {
            price: getProPriceId(period),
            quantity: 1,
          },
        ],
        mode: "subscription",
        ...(promotionCodeId
          ? { discounts: [{ promotion_code: promotionCodeId }] }
          : { allow_promotion_codes: trial ? undefined : true }),
        payment_method_collection: trial
          ? WEB_TRIAL_CHECKOUT_FIELDS.payment_method_collection
          : undefined,
        metadata: {
          checkout_type: checkoutType,
          source,
          user_id: user.id,
        },
        subscription_data: {
          metadata: {
            checkout_type: checkoutType,
            source,
            user_id: user.id,
          },
          ...(trial
            ? {
                ...WEB_TRIAL_CHECKOUT_FIELDS.subscription_data,
                ...(trialDays ? { trial_period_days: trialDays } : {}),
              }
            : {}),
        },
      },
      trial ? { idempotencyKey: `trial-checkout-${reservationId}` } : undefined,
    );
  } catch (error) {
    if (trial) {
      throw new TrialCheckoutCreationError(error);
    }
    throw error;
  }

  void captureServerAnalytics({
    event: "checkout_started",
    userId: user.id,
    insertId: `checkout-started:${checkout.id}`,
    properties: {
      plan: "pro",
      period,
      checkout_type: checkoutType,
      entry_point: source,
    },
  }).catch((error) => {
    captureOperationalError(error, {
      operation: "checkout_analytics_capture",
      level: "warning",
    });
  });

  return { url: checkout.url, stripeCustomerId };
}

const createCheckoutSessionInput = z.object({
  period: z.enum(["monthly", "yearly"]),
  plan: z.enum(["pro"]).default("pro").optional(),
  scheme: desktopSchemeSchema.optional(),
  trial: z.boolean().default(false),
  source: checkoutSourceSchema.default("unknown"),
  returnTo: z.string().optional(),
  code: z
    .string()
    .trim()
    .max(64)
    .optional()
    .transform((value) => (value ? value : undefined)),
});

export const createCheckoutSession = createServerFn({ method: "POST" })
  .inputValidator(createCheckoutSessionInput)
  .handler(async ({ data }) => {
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();

    if (!user?.id || user.is_anonymous) {
      throw new Error("Unauthorized");
    }

    const returnTo = sanitizeInternalReturnPath(data.returnTo);

    let reservationId: string | undefined;
    let trialDays: number | undefined;
    if (data.trial) {
      await claimPendingReferral(supabase);
      const { data: reservations, error } = await supabase.rpc(
        "reserve_pro_trial",
        { p_channel: "web" },
      );
      const reservation = Array.isArray(reservations)
        ? reservations[0]
        : undefined;
      const parsedReservationId = z
        .string()
        .uuid()
        .safeParse(reservation?.reservation_id);

      if (error || reservations?.length !== 1 || !parsedReservationId.success) {
        throw new Error("Trial is not available for this account");
      }
      reservationId = parsedReservationId.data;
      trialDays = (await getReferralTrialDays(supabase)) ?? undefined;
    }

    try {
      const stripe = getStripeClient();
      const checkoutCode =
        !data.trial && data.code ? data.code.trim() : undefined;
      let ycPromotion: Awaited<
        ReturnType<typeof findYcPromotionCodeByCustomerCode>
      > = null;

      if (checkoutCode) {
        if (!isYcPromotionCode(checkoutCode)) {
          return { url: getAccountYcPerkUrl(data.scheme, "invalid") };
        }
        ycPromotion = await findYcPromotionCodeByCustomerCode(
          stripe,
          normalizeYcPromotionCode(checkoutCode),
        );
        if (!ycPromotion) {
          return { url: getAccountYcPerkUrl(data.scheme, "invalid") };
        }
      }

      const stripeCustomerId = await getStripeCustomerIdForUser(
        supabase,
        stripe,
        user,
      );

      if (stripeCustomerId) {
        const activeSubscription = await getCurrentSubscription(
          stripe,
          stripeCustomerId,
        );

        if (activeSubscription) {
          if (reservationId) {
            await releaseTrialReservation(user.id, reservationId);
          }

          if (ycPromotion) {
            const result = await applyYcPromotionToCustomer({
              stripe,
              customerId: stripeCustomerId,
              promotion: ycPromotion,
            });
            if (result.status === "claimed") {
              return { url: getAccountYcPerkUrl(data.scheme, "claimed") };
            }
            if (
              result.status === "applied" ||
              result.status === "already_applied"
            ) {
              return { url: getAccountYcPerkUrl(data.scheme, "applied") };
            }
          } else {
            const returnUrl = data.scheme
              ? `${getBillingReturnUrl(data.scheme)}&source=${data.source}`
              : toAbsoluteInternalReturnUrl(getRequestAppOrigin(), returnTo);
            const portalSession = await stripe.billingPortal.sessions.create({
              customer: stripeCustomerId,
              return_url: returnUrl,
              ...(activeSubscription.status === "trialing"
                ? { flow_data: paymentMethodUpdateFlow(returnUrl) }
                : {}),
            });
            return { url: portalSession.url };
          }
        }
      }

      return await createCheckoutUrl({
        supabase,
        user,
        period: data.period,
        scheme: data.scheme,
        trial: data.trial,
        reservationId,
        trialDays,
        source: data.source,
        returnTo,
        promotionCodeId: ycPromotion?.id,
      });
    } catch (error) {
      if (reservationId && !(error instanceof TrialCheckoutCreationError)) {
        await releaseTrialReservation(user.id, reservationId).catch(
          (releaseError) => {
            captureOperationalError(releaseError, {
              operation: "trial_reservation_release",
            });
          },
        );
      }
      if (error instanceof TrialCheckoutCreationError) {
        throw error.checkoutError;
      }
      throw error;
    }
  });

const createTeamCheckoutSessionInput = z.object({
  workspaceId: z.string().uuid(),
  period: z.enum(["monthly", "yearly"]),
  quantity: z.number().int().positive().max(999_999),
  scheme: desktopSchemeSchema.optional(),
  returnTo: z.string().optional(),
});

export const createTeamCheckoutSession = createServerFn({ method: "POST" })
  .inputValidator(createTeamCheckoutSessionInput)
  .handler(async ({ data }) => {
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();

    if (!user?.id || user.is_anonymous) {
      throw new Error("Unauthorized");
    }

    const stripe = getStripeClient();
    const admin = getSupabaseAdminClient();
    const customerProvisioningAttemptId = crypto.randomUUID();
    const returnTo = sanitizeInternalReturnPath(data.returnTo);
    const appOrigin = getRequestAppOrigin();
    const successReturnPath = addInternalReturnPathSearch(returnTo, {
      success: "true",
      checkout: "paid",
    });
    const cancelReturnPath = addInternalReturnPathSearch(returnTo, {
      checkout: "canceled",
      checkout_type: "paid",
    });
    const returnUrl = data.scheme
      ? getBillingReturnUrl(data.scheme)
      : toAbsoluteInternalReturnUrl(appOrigin, returnTo);
    const successUrl = data.scheme
      ? `${getBillingReturnUrl(data.scheme)}&checkout=paid`
      : toAbsoluteInternalReturnUrl(appOrigin, successReturnPath);
    const cancelUrl = data.scheme
      ? `${getBillingReturnUrl(data.scheme)}&checkout=canceled&checkout_type=paid`
      : toAbsoluteInternalReturnUrl(appOrigin, cancelReturnPath);

    return startWorkspaceCheckout(
      {
        workspaceId: data.workspaceId,
        period: data.period,
        quantity: data.quantity,
        successUrl,
        cancelUrl,
        returnUrl,
      },
      {
        async getContext(workspaceId) {
          const { data: rows, error } = await supabase.rpc(
            "get_workspace_billing_checkout_context",
            { p_workspace_id: workspaceId },
          );
          const row = Array.isArray(rows) ? rows[0] : undefined;
          if (error || !row) {
            throw error ?? new Error("Workspace billing is unavailable");
          }
          return {
            workspaceName: row.workspace_name,
            stripeCustomerId: row.stripe_customer_id,
            usedSeats: row.used_seats,
          } as WorkspaceCheckoutContext;
        },
        getPriceId: getProPriceId,
        async createCustomer({ workspaceId, workspaceName }) {
          return stripe.customers.create(
            {
              name: workspaceName,
              metadata: { workspaceId },
            },
            {
              idempotencyKey: `create-workspace-customer-${workspaceId}-${customerProvisioningAttemptId}`,
            },
          );
        },
        async bindCustomer(workspaceId, customerId) {
          const { data: rows, error } = await admin.rpc(
            "sync_workspace_stripe_billing",
            {
              p_workspace_id: workspaceId,
              p_stripe_customer_id: customerId,
              p_seat_limit: null,
              p_update_seat_limit: false,
            },
          );
          if (error) throw error;
          return rows?.[0]?.assigned_customer_id as string | null | undefined;
        },
        async deleteCustomer(customerId) {
          await stripe.customers.del(customerId);
        },
        async retrieveCustomer(customerId) {
          const customer = await stripe.customers.retrieve(customerId);
          return {
            id: customer.id,
            deleted: "deleted" in customer && customer.deleted === true,
            metadata: "metadata" in customer ? customer.metadata : null,
          };
        },
        async listSubscriptions(customerId) {
          const subscriptions = await stripe.subscriptions.list({
            customer: customerId,
            status: "all",
            limit: 10,
          });
          return subscriptions.data;
        },
        async createCheckoutSession(input) {
          return stripe.checkout.sessions.create({
            customer: input.customerId,
            success_url: input.successUrl,
            cancel_url: input.cancelUrl,
            client_reference_id: input.workspaceId,
            line_items: [
              {
                price: input.priceId,
                quantity: input.quantity,
                adjustable_quantity: {
                  enabled: true,
                  minimum: input.minimumQuantity,
                  maximum: 999_999,
                },
              },
            ],
            mode: "subscription",
            metadata: {
              checkout_type: "team",
              workspace_id: input.workspaceId,
            },
            subscription_data: {
              metadata: {
                checkout_type: "team",
                workspace_id: input.workspaceId,
              },
            },
          });
        },
        async createPortalSession({ customerId, returnUrl }) {
          return stripe.billingPortal.sessions.create({
            customer: customerId,
            return_url: returnUrl,
          });
        },
      },
    );
  });

const releaseTrialReservation = async (
  userId: string,
  reservationId: string,
) => {
  const { error } = await getSupabaseAdminClient().rpc(
    "release_pro_trial_reservation",
    {
      p_user_id: userId,
      p_reservation_id: reservationId,
    },
  );
  if (error) {
    throw error;
  }
};

const createPlanSwitchSessionInput = z.object({
  targetPlan: z.enum(["pro"]).default("pro").optional(),
  targetPeriod: z.enum(["monthly", "yearly"]).default("monthly"),
  scheme: desktopSchemeSchema.optional(),
});

export const createPlanSwitchSession = createServerFn({ method: "POST" })
  .inputValidator(createPlanSwitchSessionInput)
  .handler(async ({ data }) => {
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();

    if (!user?.id || user.is_anonymous) {
      throw new Error("Unauthorized");
    }

    const stripe = getStripeClient();

    const stripeCustomerId = await getStripeCustomerIdForUser(
      supabase,
      stripe,
      user,
    );

    if (!stripeCustomerId) {
      return createCheckoutUrl({
        supabase,
        user,
        period: data.targetPeriod,
        scheme: data.scheme,
      });
    }

    const activeSubscription = await getCurrentSubscription(
      stripe,
      stripeCustomerId,
    );

    if (!activeSubscription) {
      return createCheckoutUrl({
        supabase,
        user,
        period: data.targetPeriod,
        scheme: data.scheme,
      });
    }

    if (!activeSubscription.items.data[0]) {
      return createCheckoutUrl({
        supabase,
        user,
        period: data.targetPeriod,
        scheme: data.scheme,
      });
    }

    const subscriptionItem = activeSubscription.items.data[0];
    const targetPriceId = getProPriceId(data.targetPeriod);
    const returnUrl = getBillingReturnUrl(data.scheme);

    // Stripe rejects a subscription_update_confirm flow that changes nothing.
    // Legacy desktop builds link here with the default monthly period, so a
    // monthly subscriber lands on a no-op switch.
    if (subscriptionItem.price.id === targetPriceId) {
      const portalSession = await stripe.billingPortal.sessions.create({
        customer: stripeCustomerId,
        return_url: returnUrl,
      });

      return { url: portalSession.url };
    }

    const portalSession = await stripe.billingPortal.sessions.create({
      customer: stripeCustomerId,
      return_url: returnUrl,
      flow_data: {
        type: "subscription_update_confirm",
        subscription_update_confirm: {
          subscription: activeSubscription.id,
          items: [
            {
              id: subscriptionItem.id,
              price: targetPriceId,
            },
          ],
        },
        after_completion: {
          type: "redirect",
          redirect: { return_url: returnUrl },
        },
      },
    });

    return { url: portalSession.url };
  });

const createPortalSessionInput = z.object({
  scheme: desktopSchemeSchema.optional(),
  intent: portalIntentSchema.default("manage"),
});

export const createPortalSession = createServerFn({ method: "POST" })
  .inputValidator(createPortalSessionInput)
  .handler(async ({ data }) => {
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();

    if (!user?.id) {
      throw new Error("Unauthorized");
    }

    const stripe = getStripeClient();
    const stripeCustomerId = await getStripeCustomerIdForUser(
      supabase,
      stripe,
      user,
    );

    if (!stripeCustomerId) {
      throw new Error("No Stripe customer found");
    }

    const returnUrl = getBillingReturnUrl(data.scheme);
    const portalSession = await stripe.billingPortal.sessions.create({
      customer: stripeCustomerId,
      return_url: returnUrl,
      ...(data.intent === "payment_method_update"
        ? { flow_data: paymentMethodUpdateFlow(returnUrl) }
        : {}),
    });

    return { url: portalSession.url };
  });

export const syncAfterSuccess = createServerFn({ method: "POST" }).handler(
  async () => {
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();

    if (!user?.id) {
      throw new Error("Unauthorized");
    }

    const stripe = getStripeClient();
    const stripeCustomerId = await getStripeCustomerIdForUser(
      supabase,
      stripe,
      user,
    );

    if (!stripeCustomerId) {
      return { status: "none" };
    }

    const subscription = await getCurrentSubscription(stripe, stripeCustomerId);

    if (!subscription) {
      return { status: "none" };
    }

    return {
      subscriptionId: subscription.id,
      status: subscription.status,
      priceId: subscription.items.data[0]?.price.id ?? null,
      cancelAtPeriodEnd: subscription.cancel_at_period_end,
      currentPeriodEnd: getSubscriptionAccessEnd(subscription),
    };
  },
);

export const getAccountSubscription = createServerFn({ method: "GET" }).handler(
  async () => {
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();

    if (!user?.id) {
      throw new Error("Unauthorized");
    }

    const stripe = getStripeClient();
    const stripeCustomerId = await getStripeCustomerIdForUser(
      supabase,
      stripe,
      user,
    );

    if (!stripeCustomerId) {
      return {
        cancelAtPeriodEnd: false,
        currentPeriodEnd: null,
        hasYcPerk: false,
      };
    }

    const subscription = await getCurrentSubscription(
      stripe,
      stripeCustomerId,
      { expandDiscounts: true },
    );

    if (!subscription) {
      return {
        cancelAtPeriodEnd: false,
        currentPeriodEnd: null,
        hasYcPerk: false,
      };
    }

    return {
      cancelAtPeriodEnd: subscription.cancel_at_period_end,
      currentPeriodEnd: getSubscriptionAccessEnd(subscription),
      hasYcPerk: subscriptionHasYcPerk(subscription),
    };
  },
);

export const canStartTrial = createServerFn({ method: "POST" }).handler(
  async () => {
    const supabase = getSupabaseServerClient();
    const { data: sessionData } = await supabase.auth.getSession();

    if (!sessionData.session) {
      return false;
    }

    const client = createClient({
      baseUrl: env.VITE_API_URL,
      headers: {
        Authorization: `Bearer ${sessionData.session.access_token}`,
      },
    });

    const { data, error } = await canStartTrialApi({ client });

    if (error) {
      captureOperationalError(error, {
        operation: "trial_eligibility_check",
      });
      return false;
    }

    return data?.canStartTrial ?? false;
  },
);

export const deleteAccount = createServerFn({ method: "POST" }).handler(
  async () => {
    const supabase = getSupabaseServerClient();
    const { data: sessionData } = await supabase.auth.getSession();

    if (!sessionData.session) {
      throw new Error("Not authenticated");
    }

    const client = createClient({
      baseUrl: env.VITE_API_URL,
      headers: {
        Authorization: `Bearer ${sessionData.session.access_token}`,
      },
    });

    const { error } = await deleteAccountApi({ client });
    if (error) {
      throw new Error("Failed to delete account");
    }

    await supabase.auth.signOut({ scope: "local" });
    return { success: true };
  },
);
