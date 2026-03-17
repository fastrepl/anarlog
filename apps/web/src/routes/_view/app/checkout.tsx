import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createCheckoutSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";

const validateSearch = z.object({
  period: z.enum(["monthly", "yearly"]).catch("monthly"),
  scheme: desktopSchemeSchema.optional(),
});

export const Route = createFileRoute("/_view/app/checkout")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    const { url } = await createCheckoutSession({
      data: { period: search.period, scheme: search.scheme },
    });

    if (url) {
      throw redirect({ href: url } as any);
    }

    throw redirect({ to: "/app/account/" });
  },
});
