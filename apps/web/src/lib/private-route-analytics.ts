import { env } from "@/env";
import { hasGlobalPrivacyControl } from "@/lib/global-privacy-control";

import type { AnalyticsIdentity } from "./private-route-analytics-identity";
import {
  ANALYTICS_IDENTITY_COOKIE,
  ANALYTICS_IDENTITY_MAX_AGE_SECONDS,
  createPrivateRouteIdentity,
  parseAnalyticsIdentity,
  parsePostHogDistinctId,
  serializeAnalyticsIdentity,
} from "./private-route-analytics-identity";

function readCookie(name: string) {
  const prefix = `${name}=`;
  const match = document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(prefix));
  return match ? match.slice(prefix.length) : null;
}

const privateRouteIdentity = createPrivateRouteIdentity({
  read: () => {
    try {
      const raw = readCookie(ANALYTICS_IDENTITY_COOKIE);
      return parseAnalyticsIdentity(raw ? decodeURIComponent(raw) : null);
    } catch {
      return {};
    }
  },
  write: (identity: AnalyticsIdentity) => {
    try {
      const value = encodeURIComponent(serializeAnalyticsIdentity(identity));
      const secure = window.location.protocol === "https:" ? "; secure" : "";
      document.cookie = `${ANALYTICS_IDENTITY_COOKIE}=${value}; path=/; max-age=${ANALYTICS_IDENTITY_MAX_AGE_SECONDS}; samesite=lax${secure}`;
    } catch {}
  },
});

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
 * Drops the analytics identity so the next person on this browser starts fresh.
 *
 * Without the posthog-js reset its anonymous `distinct_id` outlives the session,
 * and every later visitor's pageviews keep landing on the person we just signed
 * out. Runs regardless of Global Privacy Control or dev mode: clearing identity
 * state is never the thing we want to suppress.
 */
export function resetPrivateRouteAnalyticsIdentity() {
  if (typeof window === "undefined") {
    return;
  }

  privateRouteIdentity.reset();
  void import("posthog-js")
    .then(({ default: client }) => client.reset())
    .catch(() => undefined);
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
