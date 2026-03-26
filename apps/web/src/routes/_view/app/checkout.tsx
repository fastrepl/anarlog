import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createCheckoutSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";

const validateSearch = z.preprocess(
  (input) => {
    if (!input || typeof input !== "object") {
      return input;
    }

    const data = input as Record<string, unknown>;
    return {
      ...data,
      plan: data.plan === "lite" ? "lite" : "pro",
      period:
        data.period === "monthly" || data.period === "yearly"
          ? data.period
          : "monthly",
    };
  },
  z.discriminatedUnion("plan", [
    z.object({
      plan: z.literal("lite"),
      period: z.literal("monthly"),
      scheme: desktopSchemeSchema.optional(),
    }),
    z.object({
      plan: z.literal("pro"),
      period: z.enum(["monthly", "yearly"]),
      scheme: desktopSchemeSchema.optional(),
    }),
  ]),
);

export const Route = createFileRoute("/_view/app/checkout")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    const { url } = await createCheckoutSession({
      data: { plan: search.plan, period: search.period, scheme: search.scheme },
    });

    if (url) {
      throw redirect({ href: url } as any);
    }

    throw redirect({ to: "/app/account/" });
  },
});
