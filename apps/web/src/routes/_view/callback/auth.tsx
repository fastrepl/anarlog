import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { jwtDecode } from "jwt-decode";
import { CheckIcon, CopyIcon } from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { z } from "zod";

import { deriveBillingInfo, type SupabaseJwtPayload } from "@hypr/supabase";
import { cn } from "@hypr/utils";

import { CharLogo } from "@/components/sidebar";
import { exchangeOAuthCode, exchangeOtpToken } from "@/functions/auth";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { useAnalytics } from "@/hooks/use-posthog";
import {
  getDesktopAttributionAliasCandidates,
  getDesktopAttributionDistinctId,
} from "@/lib/desktop-attribution";

const validateSearch = z.object({
  code: z.string().optional(),
  token_hash: z.string().optional(),
  type: z
    .enum([
      "email",
      "recovery",
      "magiclink",
      "signup",
      "invite",
      "email_change",
    ])
    .optional(),
  flow: z.enum(["desktop", "web"]).default("desktop"),
  scheme: desktopSchemeSchema.catch("hyprnote"),
  redirect: z.string().optional(),
  access_token: z.string().optional(),
  refresh_token: z.string().optional(),
  web_distinct_id: z.string().optional(),
  error: z.string().optional(),
  error_code: z.string().optional(),
  error_description: z.string().optional(),
});

export const Route = createFileRoute("/_view/callback/auth")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ search }) => {
    if (search.flow === "web" && search.code) {
      const result = await exchangeOAuthCode({
        data: { code: search.code, flow: "web" },
      });

      if (result.success) {
        if (search.type === "recovery") {
          throw redirect({ to: "/update-password/", search: {} });
        }
        throw redirect({
          to: search.redirect || "/app/account/",
          search: {},
        });
      } else {
        console.error(result.error);
      }
    }

    if (search.flow === "desktop" && search.code) {
      const result = await exchangeOAuthCode({
        data: { code: search.code, flow: "desktop" },
      });

      if (result.success) {
        throw redirect({
          to: "/callback/auth/",
          search: {
            flow: "desktop",
            scheme: search.scheme,
            access_token: result.access_token,
            refresh_token: result.refresh_token,
          },
        });
      } else {
        console.error(result.error);
      }
    }

    if (search.token_hash && search.type) {
      if (search.type === "recovery") {
        const result = await exchangeOtpToken({
          data: {
            token_hash: search.token_hash,
            type: search.type,
            flow: search.flow,
          },
        });

        if (result.success) {
          throw redirect({ to: "/update-password/", search: {} });
        } else {
          console.error(result.error);
        }
      } else {
        const result = await exchangeOtpToken({
          data: {
            token_hash: search.token_hash,
            type: search.type,
            flow: search.flow,
          },
        });

        if (result.success) {
          if (search.flow === "web") {
            throw redirect({
              to: search.redirect || "/app/account/",
              search: {},
            });
          }

          if (search.flow === "desktop") {
            throw redirect({
              to: "/callback/auth/",
              search: {
                flow: "desktop",
                scheme: search.scheme,
                access_token: result.access_token,
                refresh_token: result.refresh_token,
              },
            });
          }
        } else {
          console.error(result.error);
        }
      }
    }
  },
});

function Container({ children }: { children: React.ReactNode }) {
  const contentRef = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState<number | "auto">("auto");

  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      setHeight(entry.contentRect.height);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div
      className={cn([
        "flex min-h-screen items-center justify-center",
        "bg-page",
        "bg-dotted-dark",
      ])}
    >
      <div className="border-color-brand surface mx-auto w-md min-w-[320px] overflow-hidden rounded-xl border shadow-md">
        <motion.div
          animate={{ height }}
          transition={{ duration: 0.3, ease: "easeInOut" }}
        >
          <div ref={contentRef}>{children}</div>
        </motion.div>
      </div>
    </div>
  );
}

function Header({ title }: { title: string }) {
  return (
    <div className="mb-8 text-center">
      <div
        className={cn([
          "mx-auto mb-8 p-8",
          "flex items-center justify-between",
          "border-color-brand border-b",
        ])}
      >
        <CharLogo compact className="text-fg h-10 w-auto" />
        <h1 className="text-fg py-4 font-mono text-xl">{title}</h1>
      </div>
    </div>
  );
}

function Component() {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const {
    alias: aliasPosthog,
    identify: identifyPosthog,
    getDistinctId,
  } = useAnalytics();
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!search.access_token) return;

    try {
      const payload = jwtDecode<SupabaseJwtPayload>(search.access_token);
      const email = payload.email;
      const userId = payload.sub;

      if (userId) {
        const currentDistinctId = getDistinctId();
        const aliasCandidates =
          getDesktopAttributionAliasCandidates(currentDistinctId);
        for (const distinctId of aliasCandidates) {
          if (distinctId !== userId) {
            aliasPosthog(userId, distinctId);
          }
        }

        const billing = deriveBillingInfo(payload);
        identifyPosthog(userId, {
          ...(email ? { email } : {}),
          plan: billing.plan,
          trial_end_date: billing.trialEnd?.toISOString() ?? null,
        });
      }
    } catch (e) {
      console.error("Failed to decode JWT for identify:", e);
    }
  }, [aliasPosthog, getDistinctId, identifyPosthog, search.access_token]);

  const getDeeplink = () => {
    if (search.access_token && search.refresh_token) {
      const params = new URLSearchParams();
      params.set("access_token", search.access_token);
      params.set("refresh_token", search.refresh_token);
      const webDistinctId =
        search.web_distinct_id ??
        getDesktopAttributionDistinctId(getDistinctId());
      if (webDistinctId) {
        params.set("web_distinct_id", webDistinctId);
      }
      return `${search.scheme}://auth/callback?${params.toString()}`;
    }
    return null;
  };

  // Browsers require a user gesture (click) to open custom URL schemes.
  // Auto-triggering via setTimeout fails for email magic links because
  // the page is opened from an external context (email client) without
  // "transient user activation". OAuth redirects work because they maintain
  // activation through the redirect chain.
  const handleDeeplink = () => {
    const deeplink = getDeeplink();
    if (search.flow === "desktop" && deeplink) {
      window.location.href = deeplink;
    }
  };

  const handleCopy = async () => {
    const deeplink = getDeeplink();
    if (deeplink) {
      await navigator.clipboard.writeText(deeplink);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  useEffect(() => {
    if (search.flow === "web" && !search.error) {
      navigate({
        to: search.redirect || "/app/account/",
        search: {},
        replace: true,
      });
    }
  }, [search, navigate]);

  if (search.error) {
    return (
      <Container>
        <Header title="Sign-in failed" />
        <div className="flex flex-col gap-4 px-8 pb-8">
          <p className="text-fg-muted text-center">
            {search.error_description
              ? search.error_description.replaceAll("+", " ")
              : "Something went wrong during sign-in"}
          </p>

          <a
            href={`/auth?flow=${search.flow}&scheme=${search.scheme}`}
            className={cn([
              "w-full cursor-pointer px-4 py-2",
              "bg-fg hover:bg-fg/80 rounded-full font-sans text-white",
              "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
              "transition-colors",
              "flex items-center justify-center",
            ])}
          >
            Try again
          </a>
        </div>
      </Container>
    );
  }

  if (search.flow === "desktop") {
    const hasTokens = search.access_token && search.refresh_token;

    return (
      <Container>
        <Header title={hasTokens ? "Sign-in successful" : "Signing in..."} />
        <div className="flex flex-col gap-4 px-8 pb-8">
          <p className="text-fg-muted text-center">
            {hasTokens
              ? "Click the button below to return to the app"
              : "Please wait while we complete the sign-in"}
          </p>

          {hasTokens && (
            <div className="flex flex-col gap-3">
              <button
                onClick={handleDeeplink}
                className={cn([
                  "w-full cursor-pointer px-4 py-2",
                  "bg-fg hover:bg-fg/80 rounded-full font-sans text-white",
                  "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
                  "transition-colors",
                  "flex items-center justify-center",
                ])}
              >
                Open Char
              </button>

              <button
                onClick={handleCopy}
                className={cn([
                  "flex w-full cursor-pointer flex-col items-center gap-3 p-4 text-left",
                  "border-color-brand rounded-lg border",
                  "hover:bg-brand-dark/10 transition-colors",
                ])}
              >
                <p className="text-fg-muted text-sm">
                  Button not working? Copy the link instead
                </p>
                <span
                  className={cn([
                    "flex w-full items-center justify-center gap-2 px-4 py-2 font-sans text-sm",
                    "border-color-brand text-fg rounded-full border",
                    "hover:bg-brand-dark/10 transition-colors",
                  ])}
                >
                  {copied ? (
                    <>
                      <CheckIcon className="size-4" />
                      Copied!
                    </>
                  ) : (
                    <>
                      <CopyIcon className="size-4" />
                      Copy URL
                    </>
                  )}
                </span>
              </button>
            </div>
          )}
        </div>
      </Container>
    );
  }

  if (search.flow === "web") {
    return (
      <Container>
        <Header title="Redirecting..." />
        <div className="px-8 pb-8 text-center">
          <p className="text-fg-muted">Taking you to your account...</p>
        </div>
      </Container>
    );
  }
}
