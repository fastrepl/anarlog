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

type SupabaseClient = ReturnType<typeof getSupabaseServerClient>;

type AuthUser = {
  id: string;
  email?: string | null;
  user_metadata?: {
    stripe_customer_id?: string;
  } | null;
};

const assertStripeCustomerOwnership = async (
  stripe: Stripe,
  customerId: string,
  user: AuthUser,
) => {
  const customer = await stripe.customers.retrieve(customerId);
  if ("deleted" in customer && customer.deleted) {
    throw new Error("Stripe customer is unavailable");
  }
  const metadataUserIds = [
    customer.metadata?.userId,
    customer.metadata?.user_id,
    customer.metadata?.userID,
  ].filter((value): value is string => Boolean(value));
  const metadataMatches =
    metadataUserIds.length > 0 &&
    metadataUserIds.every((userId) => userId === user.id);
  const emailMatches =
    metadataUserIds.length === 0 &&
    Boolean(
      customer.email &&
      user.email &&
      customer.email.toLowerCase() === user.email.toLowerCase(),
    );
  if (!metadataMatches && !emailMatches) {
    throw new Error("Stripe customer ownership could not be verified");
  }
};

const getStripeCustomerIdForUser = async (
  supabase: SupabaseClient,
  user: AuthUser,
) => {
  const metadataCustomerId = user.user_metadata?.stripe_customer_id;

  const { data: profile, error: profileError } = await supabase
    .from("profiles")
    .select("stripe_customer_id")
    .eq("id", user.id)
    .single();

  if (profileError) {
    throw profileError;
  }

  const profileCustomerId = profile?.stripe_customer_id as
    | string
    | null
    | undefined;

  if (profileCustomerId) {
    await assertStripeCustomerOwnership(
      getStripeClient(),
      profileCustomerId,
      user,
    );
  }

  if (profileCustomerId && profileCustomerId !== metadataCustomerId) {
    await supabase.auth.updateUser({
      data: {
        stripe_customer_id: profileCustomerId,
      },
    });
  }

  return profileCustomerId;
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
  const existingStripeCustomerId = await getStripeCustomerIdForUser(supabase, {
    id: user.id,
    email: user.email,
    user_metadata: user.user_metadata,
  });

  if (existingStripeCustomerId) {
    return existingStripeCustomerId;
  }

  const stripe = getStripeClient();
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
  await assertStripeCustomerOwnership(stripe, assignedCustomerId, user);
  if (assignedCustomerId !== newCustomer.id) {
    await stripe.customers.del(newCustomer.id).catch(() => undefined);
  }

  const { error: metadataError } = await supabase.auth.updateUser({
    data: {
      stripe_customer_id: assignedCustomerId,
    },
  });
  if (metadataError) {
    console.warn("Failed to refresh Stripe customer metadata", metadataError);
  }

  return assignedCustomerId;
}

async function createCheckoutUrl({
  supabase,
  user,
  period,
  scheme,
  trial = false,
  source = "unknown",
}: {
  supabase: SupabaseClient;
  user: AuthUser & { email?: string | null };
  period: "monthly" | "yearly";
  scheme?: z.infer<typeof desktopSchemeSchema>;
  trial?: boolean;
  source?:
    | "onboarding"
    | "settings"
    | "trial_ended"
    | "feature_gate"
    | "unknown";
}) {
  const stripe = getStripeClient();
  const stripeCustomerId = await ensureStripeCustomerId(supabase, user);

  const checkoutType = trial ? "trial" : "paid";
  const successParams = new URLSearchParams({
    success: "true",
    checkout: checkoutType,
    source,
  });
  if (scheme) {
    successParams.set("scheme", scheme);
  }
  const appOrigin = getRequestAppOrigin();

  const successUrl = scheme
    ? `${getBillingReturnUrl(scheme)}&checkout=${checkoutType}&source=${source}`
    : `${appOrigin}/app/account?${successParams.toString()}`;
  const cancelUrl = scheme
    ? `${getBillingReturnUrl(scheme)}&checkout=canceled&checkout_type=${checkoutType}&source=${source}`
    : `${appOrigin}/app/account?checkout=canceled&checkout_type=${checkoutType}&source=${source}`;

  const checkout = await stripe.checkout.sessions.create({
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
    payment_method_collection: trial ? "always" : undefined,
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
            trial_period_days: 14,
            trial_settings: {
              end_behavior: { missing_payment_method: "cancel" as const },
            },
          }
        : {}),
    },
  });

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

    if (data.trial) {
      const { data: sessionData } = await supabase.auth.getSession();
      const accessToken = sessionData.session?.access_token;

      if (!accessToken) {
        throw new Error("Unauthorized");
      }

      const client = createClient({
        baseUrl: env.VITE_API_URL,
        headers: { Authorization: `Bearer ${accessToken}` },
      });
      const { data: eligibility, error } = await canStartTrialApi({ client });

      if (error || !eligibility?.canStartTrial) {
        throw new Error("Trial is not available for this account");
      }
    }

    const stripe = getStripeClient();

    const stripeCustomerId = await getStripeCustomerIdForUser(supabase, {
      id: user.id,
      email: user.email,
      user_metadata: user.user_metadata,
    });

    if (stripeCustomerId) {
      const activeSubscription = await getCurrentSubscription(
        stripe,
        stripeCustomerId,
      );

      if (activeSubscription) {
        const portalSession = await stripe.billingPortal.sessions.create({
          customer: stripeCustomerId,
          return_url: getBillingReturnUrl(data.scheme),
        });
        return { url: portalSession.url };
      }
    }

    return createCheckoutUrl({
      supabase,
      user: {
        id: user.id,
        email: user.email,
        user_metadata: user.user_metadata,
      },
      period: data.period,
      scheme: data.scheme,
      trial: data.trial,
      source: data.source,
    });
  });

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

    const stripeCustomerId = await getStripeCustomerIdForUser(supabase, {
      id: user.id,
      email: user.email,
      user_metadata: user.user_metadata,
    });

    if (!stripeCustomerId) {
      return createCheckoutUrl({
        supabase,
        user: {
          id: user.id,
          email: user.email,
          user_metadata: user.user_metadata,
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
          user_metadata: user.user_metadata,
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
          user_metadata: user.user_metadata,
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

    const stripeCustomerId = await getStripeCustomerIdForUser(supabase, {
      id: user.id,
      email: user.email,
      user_metadata: user.user_metadata,
    });

    if (!stripeCustomerId) {
      throw new Error("No Stripe customer found");
    }

    const stripe = getStripeClient();

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

    const stripeCustomerId = await getStripeCustomerIdForUser(supabase, {
      id: user.id,
      email: user.email,
      user_metadata: user.user_metadata,
    });

    if (!stripeCustomerId) {
      return { status: "none" };
    }

    const stripe = getStripeClient();

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
