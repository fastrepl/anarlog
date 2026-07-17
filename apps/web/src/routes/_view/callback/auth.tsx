import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { jwtDecode } from "jwt-decode";
import { CheckIcon, CopyIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { z } from "zod";

import { deriveBillingInfo, type SupabaseJwtPayload } from "@hypr/supabase";

import {
  AuthShell,
  authNoticeClassName,
  authPrimaryButtonClassName,
  authSecondaryButtonClassName,
} from "@/components/auth-shell";
import { exchangeOAuthCode, exchangeOtpToken } from "@/functions/auth";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { useAnalytics } from "@/hooks/use-posthog";
import {
  buildPostAuthDestination,
  sanitizeInternalReturnPath,
} from "@/lib/auth-redirect";

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
          href: buildPostAuthDestination({
            newAccount: result.newAccount,
            returnTo: search.redirect,
          }),
        } as any);
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
              href: buildPostAuthDestination({
                newAccount: result.newAccount,
                returnTo: search.redirect,
              }),
            } as any);
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

function Component() {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const { identify: identifyPosthog } = useAnalytics();
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!search.access_token) return;

    try {
      const payload = jwtDecode<SupabaseJwtPayload>(search.access_token);
      const email = payload.email;
      const userId = payload.sub;

      if (userId) {
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
  }, [search.access_token, identifyPosthog]);

  const getDeeplink = () => {
    if (search.access_token && search.refresh_token) {
      const params = new URLSearchParams();
      params.set("access_token", search.access_token);
      params.set("refresh_token", search.refresh_token);
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
        to: sanitizeInternalReturnPath(search.redirect),
        search: {},
        replace: true,
      });
    }
  }, [search, navigate]);

  if (search.error) {
    return (
      <AuthShell
        title="Sign-in didn’t work"
        description="Your notes are safe. Try the sign-in flow again."
      >
        <div className="flex flex-col gap-4">
          <p className="text-center text-sm leading-6 text-[#756b5d]">
            {search.error_description
              ? search.error_description.replaceAll("+", " ")
              : "Something went wrong during sign-in"}
          </p>

          <a
            href={`/auth?flow=${search.flow}&scheme=${search.scheme}`}
            className={authPrimaryButtonClassName}
          >
            Try again
          </a>
        </div>
      </AuthShell>
    );
  }

  if (search.flow === "desktop") {
    const hasTokens = search.access_token && search.refresh_token;

    return (
      <AuthShell
        title={hasTokens ? "You’re signed in" : "Finishing sign-in"}
        description={
          hasTokens
            ? "Return to the desktop app to keep going."
            : "Please wait while we complete the secure handoff."
        }
      >
        <div className="flex flex-col gap-4">
          {hasTokens && (
            <div className="flex flex-col gap-3">
              <button
                onClick={handleDeeplink}
                className={authPrimaryButtonClassName}
              >
                Open Anarlog
              </button>

              <div className="rounded-xl border border-[#e5ddcf] bg-[#fbfaf7] p-4 text-center">
                <p className="mb-3 text-sm leading-6 text-[#756b5d]">
                  Button not working? Copy the link instead
                </p>
                <button
                  onClick={handleCopy}
                  className={authSecondaryButtonClassName}
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
                </button>
              </div>
            </div>
          )}

          {!hasTokens && (
            <div className={authNoticeClassName}>
              <p className="text-sm font-medium text-[#4f4940]">
                Connecting your account...
              </p>
            </div>
          )}
        </div>
      </AuthShell>
    );
  }

  if (search.flow === "web") {
    return (
      <AuthShell
        title="Taking you back"
        description="Your sign-in is complete."
      >
        <div className={authNoticeClassName}>
          <p className="text-sm font-medium text-[#4f4940]">
            Opening your account...
          </p>
        </div>
      </AuthShell>
    );
  }
}
