import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createCheckoutSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";

const validateSearch = z.object({
  period: z.enum(["monthly", "yearly"]).catch("monthly"),
  plan: z.enum(["pro"]).catch("pro").optional(),
  scheme: desktopSchemeSchema.optional(),
  trial: z
    .enum(["true", "false"])
    .catch("false")
    .transform((value) => value === "true"),
  source: z
    .enum(["onboarding", "settings", "trial_ended", "feature_gate", "unknown"])
    .catch("unknown"),
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
          trial: search.trial,
          source: search.source,
        },
      }));
    } catch (e) {
      console.error("Checkout error:", e);
    }

    if (url) {
      throw redirect({ href: url } as any);
    }

    const params = new URLSearchParams({
      checkout: "failed",
      checkout_type: search.trial ? "trial" : "paid",
      source: search.source,
    });
    if (search.scheme) {
      params.set("scheme", search.scheme);
      throw redirect({ href: `/callback/billing?${params.toString()}` } as any);
    }

    throw redirect({ href: `/app/account?${params.toString()}` } as any);
  },
});
