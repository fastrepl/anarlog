import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { cn } from "@anlg/utils";

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
  },
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
});

function Component() {
  const { scheme } = Route.useSearch();

  return (
    <div className="flex min-h-screen items-center justify-center bg-linear-to-b from-white via-stone-50/20 to-white p-6">
      <div className="flex w-full max-w-md flex-col gap-8 text-center">
        <div className="flex flex-col gap-3">
          <h1 className="font-sans text-3xl tracking-tight text-stone-700">
            We couldn't change your plan
          </h1>
          <p className="text-neutral-600">
            Your subscription was not modified. Open billing to manage your
            plan, payment method, and invoices.
          </p>
        </div>

        <a
          href={scheme ? `/app/portal?scheme=${scheme}` : "/app/portal"}
          className={cn([
            "flex h-12 w-full items-center justify-center text-base font-medium transition-all",
            "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
          ])}
        >
          Manage billing
        </a>
      </div>
    </div>
  );
}
