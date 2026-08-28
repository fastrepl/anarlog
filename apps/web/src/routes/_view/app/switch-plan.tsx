import * as stylex from "@stylexjs/stylex";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { cn } from "@anlg/utils";

import { createPlanSwitchSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { captureOperationalError } from "@/lib/error-reporting";
const styles = stylex.create({
  style1: {
    display: "flex",
    minHeight: "100vh",
    alignItems: "center",
    justifyContent: "center",
    "--tw-gradient-position": {
      default: "to bottom",
      "@supports (background-image: linear-gradient(in lab, red, red))":
        "to bottom in oklab",
    },
    backgroundImage: "linear-gradient(var(--tw-gradient-stops))",
    "--tw-gradient-from": "#fff",
    "--tw-gradient-stops": "var(--tw-gradient-position, #0000 0%, #fff 100%)",
    "--tw-gradient-via": "oklab(98.4825% -.000373036 .00126523 / .2)",
    "--tw-gradient-via-stops":
      "var(--tw-gradient-position), #0000 0%, oklab(98.4825% -.000373036 .00126523 / .2) 50%, #0000 100%",
    "--tw-gradient-to": "#fff",
    padding: "1.5rem",
  },
  style2: {
    display: "flex",
    width: "100%",
    flexDirection: "column",
    gap: "2rem",
    textAlign: "center",
  },
  style3: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style4: {
    fontFamily:
      "ui-sans-serif, system-ui, sans-serif, Apple Color Emoji, Segoe UI Emoji, Segoe UI Symbol, Noto Color Emoji",
    fontSize: "1.875rem",
    lineHeight: "2.25rem",
    color: "#44403c",
  },
  style5: {
    color: "#525252",
  },
});
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
        context: {
          target_period: search.targetPeriod,
        },
      });
    }
    if (url) {
      throw redirect({
        href: url,
      } as any);
    }
  },
  component: Component,
  head: () => ({
    meta: [
      {
        name: "robots",
        content: "noindex, nofollow",
      },
    ],
  }),
});
function Component() {
  const { scheme } = Route.useSearch();
  return (
    <div {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <div {...stylex.props(styles.style3)}>
          <h1 {...stylex.props(styles.style4)}>We couldn't change your plan</h1>
          <p {...stylex.props(styles.style5)}>
            Your subscription was not modified. Open billing to manage your
            plan, payment method, and invoices.
          </p>
        </div>

        <a
          href={scheme ? `/app/portal?scheme=${scheme}` : "/app/portal"}
          {...stylex.props([
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
