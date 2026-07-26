import type Stripe from "stripe";

export type AppBindings = {
  Variables: {
    stripeEvent: Stripe.Event;
    stripeRawBody: string;
    stripeSignature: string;
  };
};
