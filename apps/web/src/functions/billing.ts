import { createServerFn } from "@tanstack/react-start";
import type Stripe from "stripe";
import { z } from "zod";

import {
  canStartTrial as canStartTrialApi,
  deleteAccount as deleteAccountApi,
  startTrial as startTrialApi,
} from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { env, requireEnv } from "@/env";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getStripeClient } from "@/functions/stripe";
import { getSupabaseServerClient } from "@/functions/supabase";

type SupabaseClient = ReturnType<typeof getSupabaseServerClient>;

type AuthUser = {
  id: string;
  user_metadata?: {
    stripe_customer_id?: string;
  } | null;
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

  const stripeCustomerId =
    profileCustomerId ?? (metadataCustomerId as string | undefined);

  if (profileCustomerId && profileCustomerId !== metadataCustomerId) {
    await supabase.auth.updateUser({
      data: {
        stripe_customer_id: profileCustomerId,
      },
    });
  }

  return stripeCustomerId;
};

const getBillingReturnUrl = (scheme?: z.infer<typeof desktopSchemeSchema>) => {
  if (scheme) {
    return `${env.VITE_APP_URL}/callback/billing?scheme=${scheme}`;
  }

  return `${env.VITE_APP_URL}/app/account`;
};

const getTargetPriceId = ({
  plan,
  period,
}: {
  plan: "lite" | "pro";
  period: "monthly" | "yearly";
}) => {
  if (plan === "lite") {
    return requireEnv(
      env.STRIPE_LITE_MONTHLY_PRICE_ID,
      "STRIPE_LITE_MONTHLY_PRICE_ID",
    );
  }

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
    user_metadata: user.user_metadata,
  });

  if (existingStripeCustomerId) {
    return existingStripeCustomerId;
  }

  const stripe = getStripeClient();
  const newCustomer = await stripe.customers.create({
    email: user.email ?? undefined,
    metadata: {
      userId: user.id,
    },
  });

  await Promise.all([
    supabase.auth.updateUser({
      data: {
        stripe_customer_id: newCustomer.id,
      },
    }),
    supabase
      .from("profiles")
      .update({ stripe_customer_id: newCustomer.id })
      .eq("id", user.id),
  ]);

  return newCustomer.id;
}

async function createCheckoutUrl({
  supabase,
  user,
  plan,
  period,
  scheme,
}: {
  supabase: SupabaseClient;
  user: AuthUser & { email?: string | null };
  plan: "lite" | "pro";
  period: "monthly" | "yearly";
  scheme?: z.infer<typeof desktopSchemeSchema>;
}) {
  const stripe = getStripeClient();
  const stripeCustomerId = await ensureStripeCustomerId(supabase, user);

  const successParams = new URLSearchParams({ success: "true" });
  if (scheme) {
    successParams.set("scheme", scheme);
  }

  const successUrl = scheme
    ? getBillingReturnUrl(scheme)
    : `${env.VITE_APP_URL}/app/account?${successParams.toString()}`;

  const checkout = await stripe.checkout.sessions.create({
    customer: stripeCustomerId,
    success_url: successUrl,
    cancel_url: `${env.VITE_APP_URL}/app/account`,
    line_items: [
      {
        price: getTargetPriceId({ plan, period }),
        quantity: 1,
      },
    ],
    mode: "subscription",
  });

  return { url: checkout.url, stripeCustomerId };
}

const createCheckoutSessionInput = z.object({
  period: z.enum(["monthly", "yearly"]),
  plan: z.enum(["lite", "pro"]).default("pro"),
  scheme: desktopSchemeSchema.optional(),
});

export const createCheckoutSession = createServerFn({ method: "POST" })
  .inputValidator(createCheckoutSessionInput)
  .handler(async ({ data }) => {
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();

    if (!user?.id) {
      throw new Error("Unauthorized");
    }

    const stripe = getStripeClient();

    const stripeCustomerId = await getStripeCustomerIdForUser(supabase, {
      id: user.id,
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
      plan: data.plan,
      period: data.period,
      scheme: data.scheme,
    });
  });

const createPlanSwitchSessionInput = z.object({
  targetPlan: z.enum(["lite", "pro"]),
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

    if (!user?.id) {
      throw new Error("Unauthorized");
    }

    const stripe = getStripeClient();

    const stripeCustomerId = await getStripeCustomerIdForUser(supabase, {
      id: user.id,
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
        plan: data.targetPlan,
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
        plan: data.targetPlan,
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
        plan: data.targetPlan,
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
              price: getTargetPriceId({
                plan: data.targetPlan,
                period: data.targetPeriod,
              }),
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

export const startTrial = createServerFn({ method: "POST" }).handler(
  async () => {
    const supabase = getSupabaseServerClient();
    const { data: sessionData } = await supabase.auth.getSession();

    if (!sessionData.session) {
      throw new Error("Unauthorized");
    }

    const client = createClient({
      baseUrl: env.VITE_API_URL,
      headers: {
        Authorization: `Bearer ${sessionData.session.access_token}`,
      },
    });

    const { data, error } = await startTrialApi({
      client,
      query: { interval: "monthly" },
    });

    if (error) {
      throw new Error("Failed to start trial");
    }

    return { started: data?.started ?? false };
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
