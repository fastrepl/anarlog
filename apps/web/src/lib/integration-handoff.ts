import { useRef, useState } from "react";

import { useMountEffect } from "../hooks/useMountEffect.ts";

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
  return token || null;
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

    const token = getNangoSessionToken(window.location.hash);
    window.history.replaceState(
      window.history.state,
      "",
      `${window.location.pathname}${window.location.search}`,
    );
    setSessionToken(token);
  });

  return sessionToken;
}
