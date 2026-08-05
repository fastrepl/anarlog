import { getRequestHeaders } from "@tanstack/react-start/server";

import { env } from "@/env";

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

  const anonDistinctId = readPostHogAnonIdFromRequest();
  if (!anonDistinctId || anonDistinctId === userId) {
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
