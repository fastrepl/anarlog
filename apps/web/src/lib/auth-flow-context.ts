import {
  DEFAULT_DESKTOP_SCHEME,
  desktopSchemeSchema,
  type DesktopScheme,
} from "../functions/desktop-flow.ts";
import { sanitizeInternalReturnPath } from "./auth-redirect.ts";

export type AuthFlowContext = {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
};

export function shouldReuseBrowserSession(search: {
  provider?: string;
  view?: string;
}) {
  return search.provider === undefined && search.view === undefined;
}

export function resolveAuthFlowContext({
  flow,
  scheme,
  redirect,
  redirectTo,
}: {
  flow?: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  redirectTo?: string;
}): AuthFlowContext {
  const redirectContext = parseAuthCallbackUrl(redirectTo);
  const resolvedFlow = flow ?? redirectContext?.flow ?? "web";
  const resolvedScheme =
    scheme ??
    redirectContext?.scheme ??
    (resolvedFlow === "desktop" ? DEFAULT_DESKTOP_SCHEME : undefined);
  const resolvedRedirect = redirect ?? redirectContext?.redirect;

  return {
    flow: resolvedFlow,
    ...(resolvedScheme ? { scheme: resolvedScheme } : {}),
    ...(resolvedRedirect
      ? { redirect: sanitizeInternalReturnPath(resolvedRedirect) }
      : {}),
  };
}

export function toAuthFlowSearch(context: AuthFlowContext) {
  if (context.flow === "desktop") {
    return {
      flow: "desktop" as const,
      scheme: context.scheme ?? DEFAULT_DESKTOP_SCHEME,
      redirect: context.redirect,
    };
  }

  return {
    flow: "web" as const,
    scheme: context.scheme,
    redirect: context.redirect,
  };
}

function parseAuthCallbackUrl(
  value: string | undefined,
): AuthFlowContext | null {
  if (!value) {
    return null;
  }

  try {
    const url = new URL(value);
    if (
      (url.protocol !== "https:" && url.protocol !== "http:") ||
      canonicalPath(url.pathname) !== "/callback/auth"
    ) {
      return null;
    }

    const parsedFlow = url.searchParams.get("flow");
    const flow = parsedFlow === "desktop" ? "desktop" : "web";
    const parsedScheme = desktopSchemeSchema.safeParse(
      url.searchParams.get("scheme"),
    );
    const redirect = url.searchParams.get("redirect") ?? undefined;

    return {
      flow,
      ...(parsedScheme.success ? { scheme: parsedScheme.data } : {}),
      ...(redirect ? { redirect } : {}),
    };
  } catch {
    return null;
  }
}

function canonicalPath(pathname: string) {
  return pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname;
}
