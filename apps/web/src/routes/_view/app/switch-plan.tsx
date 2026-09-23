import { useMutation } from "@tanstack/react-query";
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
    let result: Awaited<ReturnType<typeof createPlanSwitchSession>> | undefined;
    try {
      result = await createPlanSwitchSession({
        data: {
          targetPlan: search.targetPlan,
          targetPeriod: search.targetPeriod,
          scheme: search.scheme,
        },
      });
    } catch (e) {
      captureOperationalError(e, {
        operation: "subscription_plan_switch",
        context: { target_period: search.targetPeriod },
      });
    }

    if (result?.url) {
      throw redirect({ href: result.url } as any);
    }
    return {
      confirmation:
        result && "confirmation" in result ? result.confirmation : null,
    };
  },
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
});

function Component() {
  const search = Route.useSearch();
  const { scheme, targetPeriod } = search;
  const { confirmation } = Route.useRouteContext();
  const confirm = useMutation({
    mutationFn: () =>
      createPlanSwitchSession({ data: { ...search, confirmed: true } }),
    onSuccess: (result) => {
      if (result.url) window.location.assign(result.url);
    },
  });
  const formatter = new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: confirmation?.currency ?? "usd",
  });
  const amount = confirmation
    ? formatter.format(
        confirmation.amountDue /
          10 ** (formatter.resolvedOptions().maximumFractionDigits ?? 2),
      )
    : null;

  return (
    <div className="flex min-h-screen items-center justify-center bg-linear-to-b from-white via-stone-50/20 to-white p-6">
      <div className="flex w-full max-w-md flex-col gap-8 text-center">
        <div className="flex flex-col gap-3">
          <h1 className="font-sans text-3xl tracking-tight text-stone-700">
            {confirmation
              ? `Switch to ${targetPeriod} billing`
              : "We couldn't change your plan"}
          </h1>
          <p className="text-neutral-600">
            {confirmation
              ? `Your Pro plan and extra device slots will switch together. Estimated invoice amount: ${amount}. Any payment due will be charged to your saved payment method when you confirm.`
              : "Your subscription was not modified. Open billing to manage your plan, payment method, and invoices."}
          </p>
        </div>

        {confirmation && (
          <button
            type="button"
            disabled={confirm.isPending}
            onClick={() => confirm.mutate()}
            className="h-12 rounded-full bg-stone-600 text-white disabled:opacity-50"
          >
            {confirm.isPending ? "Updating..." : "Confirm billing change"}
          </button>
        )}
        {confirm.isError && (
          <p role="alert" className="text-red-600">
            We couldn't complete the change. Check your payment method in
            billing and try again.
          </p>
        )}
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
