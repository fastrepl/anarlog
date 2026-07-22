import { createFileRoute, redirect } from "@tanstack/react-router";
import { CheckIcon, CopyIcon } from "lucide-react";
import { useState } from "react";
import { z } from "zod";

import {
  AuthShell,
  authNoticeClassName,
  authPrimaryButtonClassName,
  authSecondaryButtonClassName,
} from "@/components/auth-shell";
import { exchangeOAuthCode } from "@/functions/auth";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { useMountEffect } from "@/hooks/useMountEffect";
import {
  resolveAuthFlowContext,
  toAuthFlowSearch,
} from "@/lib/auth-flow-context";
import {
  buildPostAuthDestination,
  sanitizeInternalReturnPath,
} from "@/lib/auth-redirect";
import {
  prepareAuthRoutePrivacy,
  readDesktopAuthHandoff,
} from "@/lib/auth-route-privacy";

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
  flow: z.enum(["desktop", "web"]).default("web"),
  scheme: desktopSchemeSchema.catch("hyprnote"),
  redirect: z.string().optional(),
  access_token: z.string().optional(),
  refresh_token: z.string().optional(),
  handoff: z.literal("stored").optional(),
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
    const context = resolveAuthFlowContext(search);

    if (search.code) {
      const result = await exchangeOAuthCode({
        data: { code: search.code, flow: search.flow },
      });

      if (!result.success) {
        throw redirectToExchangeError(search, result.error);
      }

      if (search.type === "recovery") {
        throw redirect({
          to: "/update-password/",
          search: toAuthFlowSearch(context),
        });
      }

      if (search.flow === "web") {
        throw redirect({
          href: buildPostAuthDestination({
            newAccount: result.newAccount,
            returnTo: search.redirect,
          }),
        } as any);
      }

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

    if (search.token_hash && search.type) {
      throw redirect({
        to: "/confirm-auth/",
        search: {
          token_hash: search.token_hash,
          type: search.type,
          flow: search.flow,
          scheme: search.scheme,
          redirect: search.redirect,
        },
      });
    }

    if (search.flow === "web" && !search.error) {
      throw redirect({
        href: sanitizeInternalReturnPath(search.redirect),
      } as any);
    }
  },
});

function Component() {
  const search = Route.useSearch();
  const [copied, setCopied] = useState(false);
  const [storedHandoff, setStoredHandoff] =
    useState<ReturnType<typeof readDesktopAuthHandoff>>(null);

  useMountEffect(() => {
    prepareAuthRoutePrivacy();
    if (
      search.handoff === "stored" ||
      (search.access_token && search.refresh_token)
    ) {
      setStoredHandoff(readDesktopAuthHandoff());
    }
  });

  const accessToken = search.access_token ?? storedHandoff?.accessToken;
  const refreshToken = search.refresh_token ?? storedHandoff?.refreshToken;

  const getDeeplink = () => {
    if (accessToken && refreshToken) {
      const params = new URLSearchParams();
      params.set("access_token", accessToken);
      params.set("refresh_token", refreshToken);
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

  if (search.error) {
    const retrySearch = toAuthFlowSearch(resolveAuthFlowContext(search));
    const retryParams = new URLSearchParams({ flow: retrySearch.flow });
    if (retrySearch.scheme) retryParams.set("scheme", retrySearch.scheme);
    if (retrySearch.redirect) retryParams.set("redirect", retrySearch.redirect);

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
            href={`/auth?${retryParams.toString()}`}
            className={authPrimaryButtonClassName}
          >
            Try again
          </a>
        </div>
      </AuthShell>
    );
  }

  if (search.flow === "desktop") {
    const hasTokens = accessToken && refreshToken;

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

function redirectToExchangeError(
  search: {
    flow: "desktop" | "web";
    scheme: z.infer<typeof desktopSchemeSchema>;
    redirect?: string;
  },
  error: string,
) {
  return redirect({
    to: "/callback/auth/",
    search: {
      flow: search.flow,
      scheme: search.scheme,
      redirect: search.redirect,
      error: "exchange_failed",
      error_description: error,
    },
  });
}
