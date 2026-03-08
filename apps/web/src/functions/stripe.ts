import { createServerOnlyFn } from "@tanstack/react-start";
import Stripe from "stripe";

import { env, requireEnv } from "@/env";

export const getStripeClient = createServerOnlyFn(() => {
  return new Stripe(requireEnv(env.STRIPE_SECRET_KEY, "STRIPE_SECRET_KEY"), {
    apiVersion: "2026-01-28.clover",
  });
});
