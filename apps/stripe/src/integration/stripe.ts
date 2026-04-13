import Stripe from "stripe";

import { env } from "../env";

export const STRIPE_API_VERSION = "2026-02-25.clover";

export const stripe = new Stripe(env.STRIPE_SECRET_KEY, {
  apiVersion: STRIPE_API_VERSION,
});
