import { redirect } from "@tanstack/react-router";

import type { DesktopScheme } from "../functions/desktop-flow.ts";
import {
  resolveAuthFlowContext,
  toAuthFlowSearch,
} from "./auth-flow-context.ts";
import type { AuthSignInMethod } from "./auth-last-sign-in-method.ts";
import { buildPostAuthDestination } from "./auth-redirect.ts";
import { storeDesktopAuthHandoff } from "./auth-route-privacy.ts";

export async function completeOAuthCallback({
  search,
  exchangeOAuthCode,
  capturePrivateRouteEvent,
}: {
  search: Parameters<
    typeof import("../functions/auth").exchangeOAuthCode
  >[0]["data"] & {
    flow: "desktop" | "web";
    scheme: DesktopScheme;
    desktop_state?: string;
    redirect?: string;
  };
  exchangeOAuthCode: (
    options: Parameters<
      typeof import("../functions/auth").exchangeOAuthCode
    >[0],
  ) => ReturnType<typeof import("../functions/auth").exchangeOAuthCode>;
  capturePrivateRouteEvent: typeof import("./private-route-analytics").capturePrivateRouteEvent;
}) {
  const context = resolveAuthFlowContext(search);
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

  storeDesktopAuthHandoff(
    result.access_token,
    result.refresh_token,
    Date.now(),
    search.desktop_state,
  );
  throw redirect({
    to: "/callback/auth/",
    search: {
      flow: "desktop",
      handoff: "stored",
      scheme: search.scheme,
      method: search.method,
      desktop_state: search.desktop_state,
    },
  });
}

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

function redirectToExchangeError(
  search: {
    flow: "desktop" | "web";
    scheme: DesktopScheme;
    redirect?: string;
    desktop_state?: string;
  },
  error: string,
) {
  return redirect({
    to: "/callback/auth/",
    search: {
      flow: search.flow,
      scheme: search.scheme,
      desktop_state: search.desktop_state,
      redirect: search.redirect,
      error: "exchange_failed",
      error_description: error,
    },
  });
}
