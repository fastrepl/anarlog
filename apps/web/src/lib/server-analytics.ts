import {
  deleteCookie,
  getCookie,
  getRequestHeaders,
  setCookie,
} from "@tanstack/react-start/server";

import { env } from "@/env";
import { getRequestAppOrigin } from "@/functions/app-origin";

import type { AnalyticsIdentity } from "./private-route-analytics-identity";
import {
  ANALYTICS_IDENTITY_COOKIE,
  ANALYTICS_IDENTITY_MAX_AGE_SECONDS,
  createPrivateRouteIdentity,
  parseAnalyticsIdentity,
  serializeAnalyticsIdentity,
} from "./private-route-analytics-identity";

export async function captureServerAnalytics({
  event,
  userId,
  properties = {},
  insertId,
}: {
  event: string;
  userId: string;
  properties?: Record<string, unknown>;
  insertId?: string;
}) {
  if (!env.VITE_POSTHOG_API_KEY || process.env.NODE_ENV !== "production") {
    return;
  }

  const response = await fetch(
    `${env.VITE_POSTHOG_HOST.replace(/\/+$/, "")}/capture/`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      signal: AbortSignal.timeout(1_000),
      body: JSON.stringify({
        api_key: env.VITE_POSTHOG_API_KEY,
        event,
        properties: {
          ...properties,
          distinct_id: userId,
          $groups: { account: userId },
          ...(insertId ? { $insert_id: insertId } : {}),
          surface: "api",
          analytics_schema_version: 1,
          app_version: env.VITE_APP_VERSION ?? "unknown",
        },
      }),
    },
  );

  if (!response.ok) {
    throw new Error(`PostHog capture failed with ${response.status}`);
  }
}

function readPostHogAnonIdFromRequest() {
  if (!env.VITE_POSTHOG_API_KEY) {
    return null;
  }

  try {
    const cookieHeader: string | null = getRequestHeaders().get("cookie");
    if (!cookieHeader) {
      return null;
    }

    const name = `ph_${env.VITE_POSTHOG_API_KEY}_posthog`;
    const prefix = `${name}=`;
    const raw = cookieHeader
      .split(";")
      .map((part: string) => part.trim())
      .find((part: string) => part.startsWith(prefix))
      ?.slice(prefix.length);
    if (!raw) {
      return null;
    }

    const parsed = JSON.parse(decodeURIComponent(raw)) as unknown;
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "distinct_id" in parsed &&
      typeof parsed.distinct_id === "string" &&
      parsed.distinct_id
    ) {
      return parsed.distinct_id;
    }
  } catch {
    return null;
  }

  return null;
}

function identityCookieOptions() {
  return {
    httpOnly: false,
    maxAge: ANALYTICS_IDENTITY_MAX_AGE_SECONDS,
    path: "/",
    sameSite: "lax" as const,
    secure: getRequestAppOrigin().startsWith("https://"),
  };
}

function createRequestIdentityStore() {
  return {
    read: () => parseAnalyticsIdentity(getCookie(ANALYTICS_IDENTITY_COOKIE)),
    write: (identity: AnalyticsIdentity) => {
      setCookie(
        ANALYTICS_IDENTITY_COOKIE,
        serializeAnalyticsIdentity(identity),
        identityCookieOptions(),
      );
    },
  };
}

/**
 * Ends the signed-in analytics identity for this browser.
 *
 * A hard navigation to `/callback/signout` redirects from the server, so
 * `posthog.reset()` never runs and the posthog-js anonymous id outlives the
 * session still tied to the user who just left. Dropping the record entirely
 * would let the next sign-in merge that id into a second person, so the claim
 * is kept and a fresh anonymous id takes over for later events.
 */
export function clearServerAnalyticsIdentity() {
  const claimed = createPrivateRouteIdentity(
    createRequestIdentityStore(),
  ).signOut(readPostHogAnonIdFromRequest());

  if (!claimed) {
    deleteCookie(ANALYTICS_IDENTITY_COOKIE, { path: "/" });
  }
}

/**
 * Server-side counterpart to `identifyPrivateRouteUser`.
 *
 * Used by flows that complete during `beforeLoad` (OAuth code exchange), where
 * no browser code runs before the redirect. The posthog-js anonymous id rides
 * along on the request cookie, so the merge can be emitted from here.
 */
export async function identifyServerUserFromRequest(
  userId: string,
  properties: Record<string, unknown> = {},
) {
  if (!env.VITE_POSTHOG_API_KEY || process.env.NODE_ENV !== "production") {
    return;
  }

  // A direct `/auth` visit mints its own anonymous id when posthog-js never ran
  // before it, so the funnel identity can live in our cookie alone. Nothing
  // recorded in either place means no analytics happened here (GPC, opt-out),
  // leaving no merge to make and no identity worth recording.
  const identityStore = createRequestIdentityStore();
  const postHogDistinctId = readPostHogAnonIdFromRequest();
  const identity = identityStore.read();
  if (!postHogDistinctId && !identity.anonymousId && !identity.userId) {
    return;
  }

  const anonDistinctId = createPrivateRouteIdentity(
    identityStore,
  ).anonymousIdForIdentify(userId, postHogDistinctId);
  if (!anonDistinctId) {
    return;
  }

  try {
    await fetch(`${env.VITE_POSTHOG_HOST.replace(/\/+$/, "")}/capture/`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      signal: AbortSignal.timeout(1_000),
      body: JSON.stringify({
        api_key: env.VITE_POSTHOG_API_KEY,
        event: "$identify",
        properties: {
          ...properties,
          distinct_id: userId,
          $anon_distinct_id: anonDistinctId,
          surface: "api",
          analytics_schema_version: 1,
          app_version: env.VITE_APP_VERSION ?? "unknown",
        },
      }),
    });
  } catch {
    // identity stitching is best-effort and must never block auth
  }
}
