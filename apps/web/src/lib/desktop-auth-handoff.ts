import type { DesktopScheme } from "@/functions/desktop-flow";
import type { AuthSignInMethod } from "@/lib/auth-last-sign-in-method";

import { useMountEffect } from "../hooks/useMountEffect.ts";
import { storeDesktopAuthHandoff } from "./auth-route-privacy.ts";

const autoOpenAttempts = new WeakMap<Document, Set<string>>();

export function resolveDesktopAuthCallbackMethod(
  requestedMethod: AuthSignInMethod | undefined,
  rememberedMethod: AuthSignInMethod | null,
) {
  return requestedMethod ?? rememberedMethod ?? undefined;
}

export function buildDesktopAuthDeeplink(
  scheme: DesktopScheme,
  accessToken: string | undefined,
  refreshToken: string | undefined,
  method?: AuthSignInMethod,
  desktopState?: string,
) {
  if (!accessToken || !refreshToken) {
    return null;
  }

  const params = new URLSearchParams({
    access_token: accessToken,
    refresh_token: refreshToken,
  });
  if (method) {
    params.set("method", method);
  }
  if (desktopState) params.set("state", desktopState);
  return `${scheme}://auth/callback?${params.toString()}`;
}

export function buildDesktopAuthCallbackPath(
  accessToken: string,
  refreshToken: string,
  scheme?: DesktopScheme,
  method?: AuthSignInMethod,
  desktopState?: string,
) {
  storeDesktopAuthHandoff(accessToken, refreshToken, Date.now(), desktopState);
  const params = new URLSearchParams({
    flow: "desktop",
    handoff: "stored",
  });
  if (scheme) {
    params.set("scheme", scheme);
  }
  if (method) {
    params.set("method", method);
  }
  if (desktopState) params.set("desktop_state", desktopState);
  return `/callback/auth?${params.toString()}`;
}

export function attemptDesktopAppOpen(
  deeplink: string,
  documentRef: Document = document,
) {
  const link = documentRef.createElement("a");
  link.href = deeplink;
  link.rel = "noreferrer";
  link.hidden = true;
  link.tabIndex = -1;
  documentRef.body.append(link);
  link.click();
  link.remove();
}

export function useDesktopAppAutoOpen(deeplink: string) {
  useMountEffect(() => {
    const attempts = autoOpenAttempts.get(document) ?? new Set<string>();
    autoOpenAttempts.set(document, attempts);

    if (attempts.has(deeplink)) {
      return;
    }

    attempts.add(deeplink);
    attemptDesktopAppOpen(deeplink);
  });
}

export function getDesktopAppOpenLinkProps(deeplink: string) {
  return {
    href: deeplink,
    rel: "noreferrer",
  } as const;
}
