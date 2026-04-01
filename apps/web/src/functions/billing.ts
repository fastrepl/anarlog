import { createServerFn } from "@tanstack/react-start";
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

    let stripeCustomerId = await getStripeCustomerIdForUser(supabase, {
      id: user.id,
      user_metadata: user.user_metadata,
    });

    if (stripeCustomerId) {
      const subscriptions = await stripe.subscriptions.list({
        customer: stripeCustomerId,
        status: "all",
        limit: 1,
      });

      const activeSubscription = subscriptions.data.find((sub) =>
        ["active", "trialing"].includes(sub.status),
      );

      if (activeSubscription) {
        const portalSession = await stripe.billingPortal.sessions.create({
          customer: stripeCustomerId,
          return_url: getBillingReturnUrl(data.scheme),
        });
        return { url: portalSession.url };
      }
    }

    if (!stripeCustomerId) {
      const newCustomer = await stripe.customers.create({
        email: user.email,
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

      stripeCustomerId = newCustomer.id;
    }

    const priceId =
      data.plan === "lite"
        ? requireEnv(
            env.STRIPE_LITE_MONTHLY_PRICE_ID,
            "STRIPE_LITE_MONTHLY_PRICE_ID",
          )
        : data.period === "yearly"
          ? requireEnv(env.STRIPE_YEARLY_PRICE_ID, "STRIPE_YEARLY_PRICE_ID")
          : requireEnv(env.STRIPE_MONTHLY_PRICE_ID, "STRIPE_MONTHLY_PRICE_ID");

    const successParams = new URLSearchParams({ success: "true" });
    if (data.scheme) {
      successParams.set("scheme", data.scheme);
    }

    const successUrl = data.scheme
      ? getBillingReturnUrl(data.scheme)
      : `${env.VITE_APP_URL}/app/account?${successParams.toString()}`;

    const checkout = await stripe.checkout.sessions.create({
      customer: stripeCustomerId,
      success_url: successUrl,
      cancel_url: `${env.VITE_APP_URL}/app/account`,
      line_items: [
        {
          price: priceId,
          quantity: 1,
        },
      ],
      mode: "subscription",
    });

    return { url: checkout.url };
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
      return { url: null };
    }

    const subscriptions = await stripe.subscriptions.list({
      customer: stripeCustomerId,
      status: "all",
      limit: 1,
    });

    const activeSubscription = subscriptions.data.find((sub) =>
      ["active", "trialing"].includes(sub.status),
    );

    if (!activeSubscription) {
      return { url: null };
    }

    if (!activeSubscription.items.data[0]) {
      return { url: null };
    }

    const subscriptionItemId = activeSubscription.items.data[0].id;

    const targetPriceId =
      data.targetPlan === "lite"
        ? requireEnv(
            env.STRIPE_LITE_MONTHLY_PRICE_ID,
            "STRIPE_LITE_MONTHLY_PRICE_ID",
          )
        : data.targetPeriod === "yearly"
          ? requireEnv(env.STRIPE_YEARLY_PRICE_ID, "STRIPE_YEARLY_PRICE_ID")
          : requireEnv(env.STRIPE_MONTHLY_PRICE_ID, "STRIPE_MONTHLY_PRICE_ID");

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
