import { useRef, useState } from "react";

import { useMountEffect } from "../hooks/useMountEffect.ts";

const NANGO_HANDOFF_STORAGE_KEY = "anarlog.nango-session-handoff";
const NANGO_HANDOFF_MAX_AGE_MS = 5 * 60 * 1_000;
const NANGO_SESSION_TOKEN_MAX_LENGTH = 4_096;

type PreparedNangoHandoff = {
  createdAt: number;
  pathname: string;
  token: string;
};

export function isDesktopIntegrationHandoff({
  pathname,
  search,
}: {
  pathname: string;
  search: Record<string, unknown>;
}) {
  return (
    pathname.replace(/\/+$/, "") === "/app/integration" &&
    search.flow === "desktop" &&
    search.handoff === "nango" &&
    (search.action === "connect" || search.action === "reconnect")
  );
}

export function getNangoSessionToken(hash: string) {
  if (!hash.startsWith("#")) {
    return null;
  }

  const entries = [...new URLSearchParams(hash.slice(1)).entries()];
  if (entries.length !== 1 || entries[0]?.[0] !== "session_token") {
    return null;
  }

  const token = entries[0][1].trim();
  return token &&
    token.length <= NANGO_SESSION_TOKEN_MAX_LENGTH &&
    !/[\u0000-\u001f\u007f]/.test(token)
    ? token
    : null;
}

export function prepareNangoSessionHandoff() {
  if (typeof window === "undefined") return;

  const hasSensitiveFragment = /session(?:_|%5f)token/i.test(
    window.location.hash,
  );
  const token = getNangoSessionToken(window.location.hash);
  if (!hasSensitiveFragment) return;

  window.history.replaceState(
    window.history.state,
    "",
    `${window.location.pathname}${window.location.search}`,
  );

  const search = Object.fromEntries(
    new URLSearchParams(window.location.search).entries(),
  );
  if (
    !token ||
    !isDesktopIntegrationHandoff({
      pathname: window.location.pathname,
      search,
    })
  ) {
    return;
  }

  try {
    window.sessionStorage.setItem(
      NANGO_HANDOFF_STORAGE_KEY,
      JSON.stringify({
        createdAt: Date.now(),
        pathname: window.location.pathname.replace(/\/+$/, ""),
        token,
      } satisfies PreparedNangoHandoff),
    );
  } catch {}
}

function consumePreparedNangoSessionToken() {
  try {
    const raw = window.sessionStorage.getItem(NANGO_HANDOFF_STORAGE_KEY);
    window.sessionStorage.removeItem(NANGO_HANDOFF_STORAGE_KEY);
    if (!raw) return null;
    const handoff = JSON.parse(raw) as Partial<PreparedNangoHandoff>;
    if (
      typeof handoff.token !== "string" ||
      typeof handoff.createdAt !== "number" ||
      handoff.pathname !== window.location.pathname.replace(/\/+$/, "") ||
      Date.now() - handoff.createdAt > NANGO_HANDOFF_MAX_AGE_MS
    ) {
      return null;
    }
    return getNangoSessionToken(
      `#session_token=${encodeURIComponent(handoff.token)}`,
    );
  } catch {
    return null;
  }
}

export function useNangoSessionHandoffToken() {
  const capturedRef = useRef(false);
  const [sessionToken, setSessionToken] = useState<string | null | undefined>(
    undefined,
  );

  useMountEffect(() => {
    if (capturedRef.current) {
      return;
    }
    capturedRef.current = true;

    const search = Object.fromEntries(
      new URLSearchParams(window.location.search).entries(),
    );
    const isExpectedHandoff = isDesktopIntegrationHandoff({
      pathname: window.location.pathname,
      search,
    });
    const token = isExpectedHandoff
      ? (consumePreparedNangoSessionToken() ??
        getNangoSessionToken(window.location.hash))
      : null;
    window.history.replaceState(
      window.history.state,
      "",
      `${window.location.pathname}${window.location.search}`,
    );
    setSessionToken(token);
  });

  return sessionToken;
}
