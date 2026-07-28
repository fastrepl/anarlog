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
