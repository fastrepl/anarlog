import * as stylex from "@stylexjs/stylex";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { fonts, radii } from "@anlg/design-system/tokens.stylex";

import { createPlanSwitchSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { captureOperationalError } from "@/lib/error-reporting";
const styles = stylex.create({
  style1: {
    display: "flex",
    minHeight: "100vh",
    alignItems: "center",
    justifyContent: "center",
    backgroundImage:
      "linear-gradient(to bottom, #fff, rgb(250 250 249 / 0.2), #fff)",
    padding: "1.5rem",
  },
  style2: {
    display: "flex",
    width: "100%",
    maxWidth: "28rem",
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
    fontFamily: fonts.sans,
    fontSize: "1.875rem",
    lineHeight: "2.25rem",
    letterSpacing: "-.025em",
    color: "#44403c",
  },
  style5: {
    color: "#525252",
  },
  manageBillingLink: {
    alignItems: "center",
    backgroundImage: "linear-gradient(to top, #57534e, #78716c)",
    borderRadius: radii.full,
    boxShadow: {
      default:
        "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
      ":hover":
        "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)",
    },
    color: "#fff",
    display: "flex",
    fontSize: "1rem",
    fontWeight: 500,
    height: "3rem",
    justifyContent: "center",
    transform: {
      default: "scale(1)",
      ":hover": "scale(1.02)",
      ":active": "scale(.98)",
    },
    transitionDuration: ".15s",
    transitionProperty: "box-shadow, transform",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    width: "100%",
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
          {...stylex.props(styles.manageBillingLink)}
        >
          Manage billing
        </a>
      </div>
    </div>
  );
}
