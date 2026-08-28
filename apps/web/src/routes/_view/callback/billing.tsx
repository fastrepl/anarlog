import { Check, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { z } from "zod";

import { fonts, radii } from "@anlg/design-system/tokens.stylex";

import {
  DEFAULT_DESKTOP_SCHEME,
  desktopSchemeSchema,
} from "@/functions/desktop-flow";
import { useAnalytics } from "@/hooks/use-posthog";
import {
  buildBillingRefreshDeeplink,
  checkoutSourceSchema,
} from "@/lib/checkout-source";
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
  style6: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  style7: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#78716c",
  },
  style8: {
    width: "1rem",
    height: "1rem",
  },
  primaryButton: {
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
    cursor: "pointer",
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
  copyCard: {
    alignItems: "center",
    backgroundColor: {
      default: "#fafafa",
      ":hover": "#f5f5f5",
    },
    borderColor: "#f5f5f4",
    borderRadius: ".5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    cursor: "pointer",
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
    padding: "1rem",
    textAlign: "left",
    transform: {
      default: "scale(1)",
      ":active": "scale(.99)",
    },
    transitionDuration: ".15s",
    transitionProperty: "background-color, transform",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    width: "100%",
  },
  copyPill: {
    alignItems: "center",
    backgroundImage: "linear-gradient(to top, #e5e5e5, #f5f5f5)",
    borderRadius: radii.full,
    boxShadow: "0 1px 2px rgb(0 0 0 / 0.05)",
    color: "#171717",
    display: "flex",
    fontSize: ".875rem",
    fontWeight: 500,
    gap: ".5rem",
    height: "2.5rem",
    justifyContent: "center",
    width: "100%",
  },
});
const validateSearch = z.object({
  scheme: desktopSchemeSchema.optional(),
  checkout: z.enum(["trial", "paid", "canceled", "failed"]).optional(),
  checkout_type: z.enum(["trial", "paid"]).optional(),
  source: checkoutSourceSchema.optional(),
});
export const Route = createFileRoute("/_view/callback/billing")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    if (!search.scheme) {
      throw redirect({
        to: "/app/account/",
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
  const {
    scheme = DEFAULT_DESKTOP_SCHEME,
    checkout,
    checkout_type: checkoutType,
    source,
  } = Route.useSearch();
  const { track } = useAnalytics();
  const [copied, setCopied] = useState(false);
  const deeplink = buildBillingRefreshDeeplink({
    scheme,
    checkout,
    checkoutType,
    source,
  });
  const handleDeeplink = () => {
    window.location.href = deeplink;
  };
  const handleCopy = async () => {
    await navigator.clipboard.writeText(deeplink);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  useEffect(() => {
    if (checkout === "canceled" || checkout === "failed") {
      track(`checkout_${checkout}`, {
        checkout_type: checkoutType ?? "unknown",
        entry_source: source ?? "unknown",
      });
    }
    const timer = setTimeout(() => {
      window.location.href = deeplink;
    }, 250);
    return () => clearTimeout(timer);
  }, [checkout, checkoutType, deeplink, source, track]);
  return (
    <div {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <div {...stylex.props(styles.style3)}>
          <h1 {...stylex.props(styles.style4)}>Returning to Anarlog</h1>
          <p {...stylex.props(styles.style5)}>
            Click the button below if the app does not open automatically
          </p>
        </div>

        <div {...stylex.props(styles.style6)}>
          <button
            onClick={handleDeeplink}
            {...stylex.props(styles.primaryButton)}
          >
            Open Anarlog
          </button>

          <button onClick={handleCopy} {...stylex.props(styles.copyCard)}>
            <p {...stylex.props(styles.style7)}>
              Button not working? Copy the link instead
            </p>
            <span {...stylex.props(styles.copyPill)}>
              {copied ? (
                <>
                  <Check {...stylex.props(styles.style8)} />
                  Copied!
                </>
              ) : (
                <>
                  <Copy {...stylex.props(styles.style8)} />
                  Copy URL
                </>
              )}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
