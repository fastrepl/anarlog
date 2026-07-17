import { createServerFn } from "@tanstack/react-start";
import type Stripe from "stripe";
import { z } from "zod";

import {
  canStartTrial as canStartTrialApi,
  deleteAccount as deleteAccountApi,
} from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { env, requireEnv } from "@/env";
import { getRequestAppOrigin } from "@/functions/app-origin";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getStripeClient } from "@/functions/stripe";
import {
  getSupabaseAdminClient,
  getSupabaseServerClient,
} from "@/functions/supabase";
import {
  addInternalReturnPathSearch,
  sanitizeInternalReturnPath,
  toAbsoluteInternalReturnUrl,
} from "@/lib/auth-redirect";
import { getStripeCustomerOwnership } from "@/lib/stripe-customer";
import { WEB_TRIAL_CHECKOUT_FIELDS } from "@/lib/trial-policy";

type SupabaseClient = ReturnType<typeof getSupabaseServerClient>;

type AuthUser = {
  id: string;
  email?: string | null;
};

class TrialCheckoutCreationError extends Error {
  constructor(readonly checkoutError: unknown) {
    super("Could not create trial checkout session");
  }
}

const getStripeCustomerIdForUser = async (
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

  if (ownership === "claimable") {
    await stripe.customers.update(stripeCustomerId, {
      metadata: { userId: user.id },
    });
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

const getProPriceId = (period: "monthly" | "yearly") => {
  if (period === "yearly") {
    return requireEnv(env.STRIPE_YEARLY_PRICE_ID, "STRIPE_YEARLY_PRICE_ID");
  }

  return requireEnv(env.STRIPE_MONTHLY_PRICE_ID, "STRIPE_MONTHLY_PRICE_ID");
};

async function getCurrentSubscription(
  stripe: Stripe,
  stripeCustomerId: string,
): Promise<Stripe.Subscription | null> {
  const subscriptions = await stripe.subscriptions.list({
    customer: stripeCustomerId,
    status: "all",
    limit: 10,
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

async function createCheckoutUrl({
  supabase,
  user,
  period,
  scheme,
  trial = false,
  reservationId,
  source = "unknown",
  returnTo,
}: {
  supabase: SupabaseClient;
  user: AuthUser & { email?: string | null };
  period: "monthly" | "yearly";
  scheme?: z.infer<typeof desktopSchemeSchema>;
  trial?: boolean;
  reservationId?: string;
  source?:
    | "onboarding"
    | "settings"
    | "trial_ended"
    | "feature_gate"
    | "unknown";
  returnTo?: string;
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
          ...(trial ? WEB_TRIAL_CHECKOUT_FIELDS.subscription_data : {}),
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

  return { url: checkout.url, stripeCustomerId };
}

const createCheckoutSessionInput = z.object({
  period: z.enum(["monthly", "yearly"]),
  plan: z.enum(["pro"]).default("pro").optional(),
  scheme: desktopSchemeSchema.optional(),
  trial: z.boolean().default(false),
  source: z
    .enum(["onboarding", "settings", "trial_ended", "feature_gate", "unknown"])
    .default("unknown"),
  returnTo: z.string().optional(),
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
    if (data.trial) {
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
    }

    try {
      const stripe = getStripeClient();

      const stripeCustomerId = await getStripeCustomerIdForUser(
        supabase,
        stripe,
        { id: user.id, email: user.email },
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
          const portalSession = await stripe.billingPortal.sessions.create({
            customer: stripeCustomerId,
            return_url: data.scheme
              ? getBillingReturnUrl(data.scheme)
              : toAbsoluteInternalReturnUrl(getRequestAppOrigin(), returnTo),
          });
          return { url: portalSession.url };
        }
      }

      return await createCheckoutUrl({
        supabase,
        user: {
          id: user.id,
          email: user.email,
        },
        period: data.period,
        scheme: data.scheme,
        trial: data.trial,
        reservationId,
        source: data.source,
        returnTo,
      });
    } catch (error) {
      if (reservationId && !(error instanceof TrialCheckoutCreationError)) {
        await releaseTrialReservation(user.id, reservationId).catch(
          (releaseError) => {
            console.error("release_pro_trial_reservation error:", releaseError);
          },
        );
      }
      if (error instanceof TrialCheckoutCreationError) {
        throw error.checkoutError;
      }
      throw error;
    }
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
      { id: user.id, email: user.email },
    );

    if (!stripeCustomerId) {
      return createCheckoutUrl({
        supabase,
        user: {
          id: user.id,
          email: user.email,
        },
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
        user: {
          id: user.id,
          email: user.email,
        },
        period: data.targetPeriod,
        scheme: data.scheme,
      });
    }

    if (!activeSubscription.items.data[0]) {
      return createCheckoutUrl({
        supabase,
        user: {
          id: user.id,
          email: user.email,
        },
        period: data.targetPeriod,
        scheme: data.scheme,
      });
    }

    const subscriptionItemId = activeSubscription.items.data[0].id;

    const returnUrl = getBillingReturnUrl(data.scheme);
    const portalSession = await stripe.billingPortal.sessions.create({
      customer: stripeCustomerId,
      return_url: returnUrl,
      flow_data: {
        type: "subscription_update_confirm",
        subscription_update_confirm: {
          subscription: activeSubscription.id,
          items: [
            {
              id: subscriptionItemId,
              price: getProPriceId(data.targetPeriod),
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
      { id: user.id, email: user.email },
    );

    if (!stripeCustomerId) {
      throw new Error("No Stripe customer found");
    }

    const portalSession = await stripe.billingPortal.sessions.create({
      customer: stripeCustomerId,
      return_url: getBillingReturnUrl(data.scheme),
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
      { id: user.id, email: user.email },
    );

    if (!stripeCustomerId) {
      return { status: "none" };
    }

    const subscriptions = await stripe.subscriptions.list({
      customer: stripeCustomerId,
      status: "all",
    });

    // Prioritize active subscriptions over trialing ones
    // This ensures paid users see "active" status even if they had a previous trial
    const subscription =
      subscriptions.data.find((sub) => sub.status === "active") ||
      subscriptions.data.find((sub) => sub.status === "trialing");

    if (!subscription) {
      return { status: "none" };
    }

    return {
      subscriptionId: subscription.id,
      status: subscription.status,
      priceId: subscription.items.data[0]?.price.id ?? null,
      cancelAtPeriodEnd: subscription.cancel_at_period_end,
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
      console.error("can_start_trial error:", error);
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
