import { Check, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { useState } from "react";
import { z } from "zod";

import { AuthShell, authStyles } from "@/components/auth-shell";
import { exchangeOAuthCode } from "@/functions/auth";
import {
  DEFAULT_DESKTOP_SCHEME,
  desktopSchemeSchema,
} from "@/functions/desktop-flow";
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
  consumeDesktopAuthHandoff,
  prepareAuthRoutePrivacy,
} from "@/lib/auth-route-privacy";
import {
  buildDesktopAuthDeeplink,
  getDesktopAppOpenLinkProps,
  useDesktopAppAutoOpen,
} from "@/lib/desktop-auth-handoff";
const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  style2: {
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style3: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#4f4940",
  },
  style4: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style5: {
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#e5ddcf",
    backgroundColor: "#fbfaf7",
    padding: "1rem",
    textAlign: "center",
  },
  style6: {
    marginBottom: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style7: {
    width: "1rem",
    height: "1rem",
  },
});
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
  scheme: desktopSchemeSchema.catch(DEFAULT_DESKTOP_SCHEME),
  redirect: z.string().optional(),
  access_token: z.string().optional(),
  refresh_token: z.string().optional(),
  handoff: z.literal("stored").optional(),
  auto_open: z.literal("oauth").optional(),
  error: z.string().optional(),
  error_code: z.string().optional(),
  error_description: z.string().optional(),
});
export const Route = createFileRoute("/_view/callback/auth")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [
      {
        name: "robots",
        content: "noindex, nofollow",
      },
    ],
  }),
  beforeLoad: async ({ search }) => {
    const context = resolveAuthFlowContext(search);
    if (search.code) {
      const result = await exchangeOAuthCode({
        data: {
          code: search.code,
          flow: search.flow,
        },
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
          auto_open: "oauth",
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
  const [storedHandoff, setStoredHandoff] =
    useState<ReturnType<typeof consumeDesktopAuthHandoff>>(null);
  const accessToken = search.access_token ?? storedHandoff?.accessToken;
  const refreshToken = search.refresh_token ?? storedHandoff?.refreshToken;
  const deeplink = buildDesktopAuthDeeplink(
    search.scheme,
    accessToken,
    refreshToken,
  );
  useMountEffect(() => {
    prepareAuthRoutePrivacy();
    if (
      search.handoff === "stored" ||
      (search.access_token && search.refresh_token)
    ) {
      const handoff = consumeDesktopAuthHandoff();
      if (handoff) {
        setStoredHandoff(handoff);
      }
    }
  });
  if (search.error) {
    const retrySearch = toAuthFlowSearch(resolveAuthFlowContext(search));
    const retryParams = new URLSearchParams({
      flow: retrySearch.flow,
    });
    if (retrySearch.scheme) retryParams.set("scheme", retrySearch.scheme);
    if (retrySearch.redirect) retryParams.set("redirect", retrySearch.redirect);
    return (
      <AuthShell
        title="Sign-in didn’t work"
        description="Your notes are safe. Try the sign-in flow again."
      >
        <div {...stylex.props(styles.style1)}>
          <p {...stylex.props(styles.style2)}>
            {search.error_description
              ? search.error_description.replaceAll("+", " ")
              : "Something went wrong during sign-in"}
          </p>

          <a
            href={`/auth?${retryParams.toString()}`}
            {...stylex.props(authStyles.primaryButton)}
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
        <div {...stylex.props(styles.style1)}>
          {deeplink && <DesktopAuthHandoffActions deeplink={deeplink} />}

          {!hasTokens && (
            <div {...stylex.props(authStyles.notice)}>
              <p {...stylex.props(styles.style3)}>Connecting your account...</p>
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
        <div {...stylex.props(authStyles.notice)}>
          <p {...stylex.props(styles.style3)}>Opening your account...</p>
        </div>
      </AuthShell>
    );
  }
}
function DesktopAuthHandoffActions({ deeplink }: { deeplink: string }) {
  const [copied, setCopied] = useState(false);
  useDesktopAppAutoOpen(deeplink);
  const handleCopy = async () => {
    await navigator.clipboard.writeText(deeplink);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  return (
    <div {...stylex.props(styles.style4)}>
      <a
        {...getDesktopAppOpenLinkProps(deeplink)}
        {...stylex.props(authStyles.primaryButton)}
      >
        Open Anarlog
      </a>

      <div {...stylex.props(styles.style5)}>
        <p {...stylex.props(styles.style6)}>
          Button not working? Copy the link instead
        </p>
        <button
          onClick={handleCopy}
          {...stylex.props(authStyles.secondaryButton)}
        >
          {copied ? (
            <>
              <Check {...stylex.props(styles.style7)} />
              Copied!
            </>
          ) : (
            <>
              <Copy {...stylex.props(styles.style7)} />
              Copy URL
            </>
          )}
        </button>
      </div>
    </div>
  );
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
