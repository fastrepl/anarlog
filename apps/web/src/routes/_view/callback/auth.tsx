import { createFileRoute, redirect } from "@tanstack/react-router";
import { useState } from "react";
import { z } from "zod";

import { Check, Copy } from "@anlg/ui/components/icons";

import {
  AuthShell,
  authNoticeClassName,
  authPrimaryButtonClassName,
  authSecondaryButtonClassName,
} from "@/components/auth-shell";
import { finishAccountIdentity } from "@/functions/account-identities";
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
  type AuthSignInMethod,
  authSignInMethods,
} from "@/lib/auth-last-sign-in-method";
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
import { capturePrivateRouteEvent } from "@/lib/private-route-analytics";

const validateSearch = z.object({
  intent: z.literal("link_identity").optional(),
  link_state: z.string().optional(),
  account_user_id: z.uuid().optional(),
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
  method: z.enum(authSignInMethods).optional(),
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

function toAuthCompletionMethod(
  method: AuthSignInMethod | undefined,
  type: string | undefined,
) {
  switch (method) {
    case "apple":
    case "google":
    case "azure":
    case "github":
      return { method: "oauth", provider: method };
    case "sso":
      return { method: "sso" };
    case "email":
      return { method: type === "magiclink" ? "magic_link" : "email_link" };
    default:
      return { method: "code_exchange" };
  }
}

export const Route = createFileRoute("/_view/callback/auth")({
  // Exchange from a same-origin request after Safari's cross-site return.
  ssr: false,
  validateSearch,
  component: Component,
  pendingComponent: () => (
    <AuthShell
      title="Finishing sign-in"
      description="Please wait while we connect your account."
      children={null}
    />
  ),
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ search }) => {
    if (search.intent === "link_identity") {
      const hash = new URLSearchParams(window.location.hash.slice(1));
      const result = await finishAccountIdentity({
        data: {
          state: search.link_state,
          code: search.code,
          error:
            search.error_code ??
            hash.get("error_code") ??
            search.error ??
            hash.get("error") ??
            undefined,
        },
      }).catch(() => ({ status: "failed" as const, userId: undefined }));
      throw redirect({
        to: "/app/account/",
        search: {
          section: "connected-accounts",
          identity_link: result.status,
          account_user_id: result.userId ?? search.account_user_id,
        },
        hash: "connected-accounts",
      });
    }

    const context = resolveAuthFlowContext(search);

    if (search.code) {
      const result = await exchangeOAuthCode({
        data: {
          code: search.code,
          flow: search.flow,
          type: search.type,
          method: search.method,
        },
      });

      if (!result.success) {
        throw redirectToExchangeError(search, result.error);
      }

      capturePrivateRouteEvent("auth_completed", {
        ...toAuthCompletionMethod(search.method, search.type),
        action: search.type ?? "sign_in",
        flow: search.flow,
        new_account: result.createdAccount === true,
      });

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
          method: search.method,
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
          method: search.method,
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
    search.method,
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
          {deeplink && <DesktopAuthHandoffActions deeplink={deeplink} />}

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

function DesktopAuthHandoffActions({ deeplink }: { deeplink: string }) {
  const [copied, setCopied] = useState(false);

  useDesktopAppAutoOpen(deeplink);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(deeplink);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-col gap-3">
      <a
        {...getDesktopAppOpenLinkProps(deeplink)}
        className={authPrimaryButtonClassName}
      >
        Open Anarlog
      </a>

      <div className="rounded-xl border border-[#e5ddcf] bg-[#fbfaf7] p-4 text-center">
        <p className="mb-3 text-sm leading-6 text-[#756b5d]">
          Button not working? Copy the link instead
        </p>
        <button onClick={handleCopy} className={authSecondaryButtonClassName}>
          {copied ? (
            <>
              <Check className="size-4" />
              Copied!
            </>
          ) : (
            <>
              <Copy className="size-4" />
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
