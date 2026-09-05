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
  getPostHogPersistenceName,
  parseAnalyticsIdentity,
  serializeAnalyticsIdentity,
} from "./private-route-analytics-identity";
import { sendServerAnalytics } from "./server-analytics-capture";

export async function captureServerAnalytics({
  userId: _userId,
  ...event
}: Pick<
  Parameters<typeof sendServerAnalytics>[0],
  "event" | "properties" | "insertId" | "timestamp"
> & {
  userId: string;
}) {
  if (!env.VITE_POSTHOG_API_KEY || process.env.NODE_ENV !== "production") {
    return;
  }

  await sendServerAnalytics({
    ...event,
    apiKey: env.VITE_POSTHOG_API_KEY,
    host: env.VITE_POSTHOG_HOST,
    appVersion: env.VITE_APP_VERSION ?? "unknown",
  });
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

    const name = `ph_${getPostHogPersistenceName(env.VITE_POSTHOG_API_KEY)}`;
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
  _userId: string,
  _properties: Record<string, unknown> = {},
) {
  // Server auth flows intentionally do not stitch browser activity to an account.
}
