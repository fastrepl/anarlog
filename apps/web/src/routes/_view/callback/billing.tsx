import { Check, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { z } from "zod";

import { cn } from "@anlg/utils";

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
            {...stylex.props([
              "flex h-12 w-full cursor-pointer items-center justify-center text-base font-medium transition-all",
              "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
            ])}
          >
            Open Anarlog
          </button>

          <button
            onClick={handleCopy}
            {...stylex.props([
              "flex w-full cursor-pointer flex-col items-center gap-3 p-4 text-left transition-all",
              "rounded-lg border border-stone-100 bg-stone-50 hover:bg-stone-100 active:scale-[99%]",
            ])}
          >
            <p {...stylex.props(styles.style7)}>
              Button not working? Copy the link instead
            </p>
            <span
              {...stylex.props([
                "flex h-10 w-full items-center justify-center gap-2 text-sm font-medium",
                "rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs",
              ])}
            >
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
