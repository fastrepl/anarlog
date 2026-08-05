import { env } from "@/env";
import { hasGlobalPrivacyControl } from "@/lib/global-privacy-control";

import {
  createPrivateRouteIdentity,
  parsePostHogDistinctId,
} from "./private-route-analytics-identity";

const privateRouteIdentity = createPrivateRouteIdentity();

function readCookie(name: string) {
  const prefix = `${name}=`;
  const match = document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(prefix));
  return match ? match.slice(prefix.length) : null;
}

/**
 * posthog-js persists its anonymous distinct_id under `ph_<token>_posthog`
 * (localStorage, mirrored to a cookie). Telemetry is disabled on auth routes,
 * so posthog-js is never initialized here and cannot hand us the id directly.
 * Reading the persisted value keeps events fired from auth routes attached to
 * the same person as the marketing-site pageviews that preceded them.
 */
function readPostHogDistinctId() {
  if (!env.VITE_POSTHOG_API_KEY) {
    return null;
  }

  try {
    const key = `ph_${env.VITE_POSTHOG_API_KEY}_posthog`;
    const localValue = window.localStorage.getItem(key);
    if (localValue) {
      const localDistinctId = parsePostHogDistinctId(localValue);
      if (localDistinctId) {
        return localDistinctId;
      }
    }

    const cookieValue = readCookie(key);
    return cookieValue
      ? parsePostHogDistinctId(decodeURIComponent(cookieValue))
      : null;
  } catch {
    return null;
  }
}

function analyticsSuppressed() {
  return (
    typeof window === "undefined" ||
    import.meta.env.DEV ||
    !env.VITE_POSTHOG_API_KEY ||
    hasGlobalPrivacyControl()
  );
}

export function capturePrivateRouteEvent(
  event: string,
  properties: Record<string, unknown> = {},
) {
  if (analyticsSuppressed()) {
    return;
  }

  try {
    const host = env.VITE_POSTHOG_HOST.replace(/\/+$/, "");
    const distinctId = privateRouteIdentity.distinctIdForEvent(
      readPostHogDistinctId(),
    );
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

/**
 * Merges the anonymous browsing identity into the Supabase user id.
 *
 * `account_created` / `account_confirmed` are delivered server-side (Supabase
 * trigger -> outbox -> scheduled Netlify function) keyed on the Supabase user
 * uuid, while everything captured in the browser is keyed on the posthog-js
 * anonymous id. Without this $identify the two never resolve to the same
 * person and any funnel crossing signup silently under-reports.
 */
export function identifyPrivateRouteUser(
  userId: string | undefined | null,
  properties: Record<string, unknown> = {},
) {
  if (!userId || analyticsSuppressed()) {
    return;
  }

  try {
    const anonDistinctId = privateRouteIdentity.anonymousIdForIdentify(
      userId,
      readPostHogDistinctId(),
    );
    if (!anonDistinctId) {
      return;
    }

    const host = env.VITE_POSTHOG_HOST.replace(/\/+$/, "");
    void fetch(`${host}/capture/`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      keepalive: true,
      body: JSON.stringify({
        api_key: env.VITE_POSTHOG_API_KEY,
        event: "$identify",
        properties: {
          ...properties,
          distinct_id: userId,
          $anon_distinct_id: anonDistinctId,
          surface: "web",
          analytics_schema_version: 1,
          app_version: env.VITE_APP_VERSION ?? "unknown",
        },
      }),
    }).catch(() => undefined);
  } catch {}
}
