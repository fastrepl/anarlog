import { env } from "@/env";
import { hasGlobalPrivacyControl } from "@/lib/global-privacy-control";

const PRIVATE_ANALYTICS_ID_KEY = "anarlog.private-analytics-id";
let fallbackDistinctId: string | null = null;

function getDistinctId() {
  try {
    const existing = window.sessionStorage.getItem(PRIVATE_ANALYTICS_ID_KEY);
    if (existing) return existing;

    const created = crypto.randomUUID();
    window.sessionStorage.setItem(PRIVATE_ANALYTICS_ID_KEY, created);
    return created;
  } catch {
    fallbackDistinctId ??= crypto.randomUUID();
    return fallbackDistinctId;
  }
}

export function capturePrivateRouteEvent(
  event: string,
  properties: Record<string, unknown> = {},
) {
  if (
    typeof window === "undefined" ||
    import.meta.env.DEV ||
    !env.VITE_POSTHOG_API_KEY ||
    hasGlobalPrivacyControl()
  ) {
    return;
  }

  try {
    const host = env.VITE_POSTHOG_HOST.replace(/\/+$/, "");
    const distinctId = getDistinctId();
    void fetch(`${host}/capture/`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      keepalive: true,
      body: JSON.stringify({
        api_key: env.VITE_POSTHOG_API_KEY,
        event,
        properties: {
          ...properties,
          distinct_id: distinctId,
          $session_id: distinctId,
          surface: "web",
          analytics_schema_version: 1,
          app_version: env.VITE_APP_VERSION ?? "unknown",
        },
      }),
    }).catch(() => undefined);
  } catch {}
}
