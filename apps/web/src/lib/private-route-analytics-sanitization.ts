import { sanitizeAnalyticsProperties } from "./analytics-sanitization.ts";

const PRIVATE_ANALYTICS_PROPERTY_KEYS = new Set([
  "action",
  "anchor_type",
  "checkout_type",
  "entry_source",
  "failure_stage",
  "flow",
  "has_audio",
  "has_collaboration_actions",
  "method",
  "provider",
]);

export function sanitizePrivateRouteAnalyticsProperties(
  properties: Record<string, unknown>,
) {
  return sanitizeAnalyticsProperties(
    Object.fromEntries(
      Object.entries(properties).filter(([key]) =>
        PRIVATE_ANALYTICS_PROPERTY_KEYS.has(key),
      ),
    ),
  );
}
