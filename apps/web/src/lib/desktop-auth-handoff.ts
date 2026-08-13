import type { DesktopScheme } from "@/functions/desktop-flow";

import { useMountEffect } from "../hooks/useMountEffect.ts";

const autoOpenAttempts = new WeakMap<Document, Set<string>>();

export function buildDesktopAuthDeeplink(
  scheme: DesktopScheme,
  accessToken: string | undefined,
  refreshToken: string | undefined,
) {
  if (!accessToken || !refreshToken) {
    return null;
  }

  const params = new URLSearchParams({
    access_token: accessToken,
    refresh_token: refreshToken,
  });
  return `${scheme}://auth/callback?${params.toString()}`;
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
