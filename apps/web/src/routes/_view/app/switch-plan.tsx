import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createPlanSwitchSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { captureOperationalError } from "@/lib/error-reporting";

const validateSearch = z.object({
  targetPlan: z.enum(["pro"]).catch("pro").optional(),
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
      captureOperationalError(e, {
        operation: "subscription_plan_switch",
        context: { target_period: search.targetPeriod },
      });
    }

    if (url) {
      throw redirect({ href: url } as any);
    }

    throw redirect({ to: "/app/account/" });
  },
});
