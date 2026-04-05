import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createPlanSwitchSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";

const validateSearch = z.object({
  targetPlan: z.enum(["lite", "pro"]),
  targetPeriod: z.enum(["monthly", "yearly"]).catch("monthly"),
  scheme: desktopSchemeSchema.optional(),
});

export const Route = createFileRoute("/_view/app/switch-plan")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    let url: string | null | undefined;
    try {
      ({ url } = await createPlanSwitchSession({
        data: {
          targetPlan: search.targetPlan,
          targetPeriod: search.targetPeriod,
          scheme: search.scheme,
        },
      }));
    } catch (e) {
      console.error("Plan switch error:", e);
    }

    if (url) {
      throw redirect({ href: url } as any);
    }

    throw redirect({ to: "/app/account/" });
  },
});
