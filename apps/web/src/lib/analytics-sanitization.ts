import type { CaptureResult } from "posthog-js";

const SENSITIVE_PROPERTY_KEYS = new Set([
  "account_id",
  "address",
  "body",
  "condition",
  "contact",
  "content",
  "customer_id",
  "diagnosis",
  "email",
  "error",
  "file_path",
  "full_name",
  "health",
  "medical",
  "message",
  "name",
  "note_title",
  "owner_id",
  "participant",
  "path",
  "patient",
  "prompt",
  "query",
  "request",
  "response",
  "session_id",
  "speaker",
  "team_id",
  "text",
  "token",
  "transcript",
  "url",
  "user_id",
  "workspace_id",
]);
const IDENTIFIER_PROPERTY_KEYS = new Set([
  "$insert_id",
  "$session_id",
  "distinct_id",
]);
const URL_PROPERTY_KEYS = new Set([
  "$current_url",
  "$initial_current_url",
  "$initial_referrer",
  "$pathname",
  "$referrer",
]);
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const SAFE_TOKEN_PATTERN = /^[a-z0-9][a-z0-9_.:{}<>/-]*$/i;
const SAFE_URL_PATTERN = /^[a-z0-9/][a-z0-9_.:{}<>/-]*$/i;
const SAFE_EVENT_NAME_PATTERN = /^[a-z0-9_$.-]+$/i;

export function sanitizeAnalyticsEventName(event: string) {
  return event.length > 0 &&
    event.length <= 64 &&
    SAFE_EVENT_NAME_PATTERN.test(event)
    ? event
    : "analytics_event";
}

export function sanitizeAnalyticsProperties(
  properties: Record<string, unknown>,
) {
  const sanitized: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(properties)) {
    if (isSensitivePropertyKey(key)) continue;
    const safeValue = sanitizeAnalyticsValue(key, value);
    if (safeValue !== undefined) {
      sanitized[key] = safeValue;
    }
  }
  return sanitized;
}

function isSensitivePropertyKey(key: string) {
  const normalized = key.toLowerCase();
  if (
    IDENTIFIER_PROPERTY_KEYS.has(normalized) ||
    URL_PROPERTY_KEYS.has(normalized)
  ) {
    return false;
  }
  return (
    SENSITIVE_PROPERTY_KEYS.has(normalized) ||
    normalized.endsWith("_email") ||
    normalized.endsWith("_id") ||
    normalized.endsWith("_path") ||
    normalized.endsWith("_url")
  );
}

function sanitizeAnalyticsValue(key: string, value: unknown): unknown {
  if (value === null || typeof value === "boolean") {
    return value;
  }
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : undefined;
  }
  if (typeof value === "string") {
    return isSafeAnalyticsString(key, value) ? value : undefined;
  }
  if (Array.isArray(value)) {
    const sanitized = value
      .map((item) => sanitizeAnalyticsValue(key, item))
      .filter((item) => item !== undefined);
    return sanitized.length === value.length ? sanitized : undefined;
  }
  if (value && typeof value === "object") {
    return sanitizeAnalyticsProperties(value as Record<string, unknown>);
  }
  return undefined;
}

function isSafeAnalyticsString(key: string, value: string) {
  if (!value || EMAIL_PATTERN.test(value)) {
    return false;
  }
  if (IDENTIFIER_PROPERTY_KEYS.has(key.toLowerCase())) {
    return value.length <= 128 && SAFE_TOKEN_PATTERN.test(value);
  }
  if (URL_PROPERTY_KEYS.has(key.toLowerCase())) {
    if (value === "$direct") {
      return isReferrerPropertyKey(key);
    }
    return (
      value.length <= 256 &&
      SAFE_URL_PATTERN.test(value) &&
      !value.includes("?") &&
      !value.includes("#")
    );
  }
  return (
    value.length <= 96 &&
    SAFE_TOKEN_PATTERN.test(value) &&
    !value.includes("/") &&
    !UUID_PATTERN.test(value) &&
    !(value.length >= 32 && /^[a-z0-9_-]+$/i.test(value))
  );
}

function isReferrerPropertyKey(key: string) {
  const normalized = key.toLowerCase();
  return normalized === "$referrer" || normalized === "$initial_referrer";
}

const POSTHOG_URL_PROPERTIES = [
  "$current_url",
  "$initial_current_url",
  "$initial_referrer",
  "$referrer",
] as const;

function normalizePath(pathname: string) {
  return pathname
    .split("/")
    .map((segment) => {
      let decoded = segment;
      try {
        decoded = decodeURIComponent(segment);
      } catch {
        return ":id";
      }
      return UUID_PATTERN.test(decoded) ||
        EMAIL_PATTERN.test(decoded) ||
        /^\d{6,}$/.test(decoded) ||
        decoded.length > 32
        ? ":id"
        : segment;
    })
    .join("/");
}

export function sanitizePostHogEvent(
  event: CaptureResult | null,
  origin: string,
) {
  if (!event) return null;
  const properties = { ...event.properties };
  for (const key of POSTHOG_URL_PROPERTIES) {
    const value = properties[key];
    if (typeof value !== "string") continue;
    if (isReferrerPropertyKey(key) && value === "$direct") continue;
    try {
      const url = new URL(value, origin);
      properties[key] = `${url.origin}${normalizePath(url.pathname)}`;
    } catch {
      delete properties[key];
    }
  }
  if (typeof properties.$pathname === "string") {
    properties.$pathname = normalizePath(
      properties.$pathname.split(/[?#]/, 1)[0],
    );
  }
  return {
    ...event,
    event: sanitizeAnalyticsEventName(event.event),
    properties: sanitizeAnalyticsProperties(properties),
  };
}
