import { createHash, randomUUID } from "node:crypto";

import {
  sanitizeAnalyticsEventName,
  sanitizeAnalyticsProperties,
} from "./analytics-sanitization.ts";

const SERVER_ANALYTICS_PROPERTY_KEYS = new Set([
  "checkout_type",
  "entry_point",
  "period",
  "plan",
]);

function createEventUuid(event: string, insertId: string) {
  const bytes = createHash("sha256")
    .update(JSON.stringify(["anarlog-server-event-v1", event, insertId]))
    .digest()
    .subarray(0, 16);
  bytes[6] = (bytes[6] & 0x0f) | 0x80;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.toString("hex");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export async function sendServerAnalytics({
  apiKey,
  host,
  appVersion,
  event,
  timestamp,
  properties = {},
  insertId,
  fetcher = fetch,
}: {
  apiKey: string;
  host: string;
  appVersion: string;
  event: string;
  timestamp: Date;
  properties?: Record<string, unknown>;
  insertId?: string;
  fetcher?: typeof fetch;
}) {
  // Retries need the same UUID, distinct ID, event, and timestamp. The identity
  // is scoped to this event so it cannot join separate checkouts to an account.
  const uuid = insertId ? createEventUuid(event, insertId) : randomUUID();
  const response = await fetcher(`${host.replace(/\/+$/, "")}/capture/`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    signal: AbortSignal.timeout(1_000),
    body: JSON.stringify({
      api_key: apiKey,
      event: sanitizeAnalyticsEventName(event),
      uuid,
      timestamp: timestamp.toISOString(),
      properties: {
        ...sanitizeAnalyticsProperties(
          Object.fromEntries(
            Object.entries(properties).filter(([key]) =>
              SERVER_ANALYTICS_PROPERTY_KEYS.has(key),
            ),
          ),
        ),
        distinct_id: uuid,
        $insert_id: uuid,
        $process_person_profile: false,
        surface: "api",
        analytics_schema_version: 1,
        app_version: appVersion,
      },
    }),
  });

  if (!response.ok) {
    throw new Error(`PostHog capture failed with ${response.status}`);
  }
}
