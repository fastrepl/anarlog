import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createCheckoutSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";

const validateSearch = z.object({
  period: z.enum(["monthly", "yearly"]).catch("monthly"),
  plan: z.enum(["lite", "pro"]).catch("pro"),
  scheme: desktopSchemeSchema.optional(),
});

export const Route = createFileRoute("/_view/app/checkout")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    let url: string | null | undefined;
    try {
      ({ url } = await createCheckoutSession({
        data: {
          period: search.period,
          plan: search.plan,
          scheme: search.scheme,
        },
      }));
    } catch (e) {
      console.error("Checkout error:", e);
    }

    if (url) {
      throw redirect({ href: url } as any);
    }

    throw redirect({ to: "/app/account/" });
  },
});
