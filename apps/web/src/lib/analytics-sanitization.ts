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
const POSTHOG_TEXT_PROPERTY_KEYS = new Set([
  "$os",
  "$os_version",
  "$browser",
  "$browser_version",
  "$browser_language",
  "$browser_language_prefix",
  "$device",
  "$device_type",
  "$timezone",
  "$timezone_offset",
  "$host",
  "$referring_domain",
  "$raw_user_agent",
  "$lib",
  "$lib_version",
  "$lib_rate_limit_remaining_tokens",
  "$screen_height",
  "$screen_width",
  "$viewport_height",
  "$viewport_width",
  "$time",
  "$sent_at",
  "$is_identified",
  "$process_person_profile",
  "$geoip_disable",
  "$recording_status",
  "$session_recording_start_reason",
  "$configured_session_timeout_ms",
  "$prev_pageview_duration",
  "$prev_pageview_last_scroll",
  "$prev_pageview_last_scroll_percentage",
  "$prev_pageview_last_content",
  "$prev_pageview_last_content_percentage",
  "$prev_pageview_max_scroll",
  "$prev_pageview_max_scroll_percentage",
  "$prev_pageview_max_content",
  "$prev_pageview_max_content_percentage",
  "$event_type",
  "$ce_version",
  "$el_text",
  "$elements_chain",
  "$active_feature_flags",
  "$console_log_recording_enabled_server_side",
  "$session_recording_network_payload_capture",
  "$session_recording_canvas_recording",
  "$replay_sample_rate",
  "$replay_minimum_duration",
  "$replay_script_config",
  "title",
]);
const POSTHOG_INITIAL_TEXT_PROPERTY_KEYS = new Set(
  [...POSTHOG_TEXT_PROPERTY_KEYS].map((key) => `$initial_${key.slice(1)}`),
);
const POSTHOG_TEXT_PROPERTY_PREFIXES = [
  "$feature/",
  "$feature_flag",
  "$web_vitals_",
  "$sdk_debug_",
  "utm_",
  "$initial_utm_",
] as const;
const POSTHOG_ANONYMOUS_ID_KEYS = new Set([
  "$device_id",
  "$window_id",
  "$pageview_id",
  "$anon_distinct_id",
]);
const POSTHOG_PATH_PROPERTY_KEYS = new Set([
  "$pathname",
  "$initial_pathname",
  "$prev_pageview_pathname",
]);
const POSTHOG_NESTED_PROPERTY_KEYS = new Set(["$set", "$set_once"]);
const POSTHOG_ELEMENT_ATTRIBUTE_KEYS = new Set([
  "tag_name",
  "nth_child",
  "nth_of_type",
  "order",
  "attr__href",
  "attr__class",
  "attr__id",
  "attr__type",
  "attr__role",
  "attr__name",
  "attr__aria-label",
  "attr__data-attr",
  "attr__data-ph-capture-attribute",
  "attr_class",
  "attr_id",
  "href",
  "text",
]);
const SAFE_QUERY_PARAMETER_KEYS = new Set([
  "utm_source",
  "utm_medium",
  "utm_campaign",
  "utm_term",
  "utm_content",
  "ref",
  "gclid",
  "gad_source",
  "gbraid",
  "wbraid",
  "fbclid",
  "msclkid",
  "twclid",
  "li_fat_id",
  "ttclid",
  "dclid",
  "igshid",
  "rdt_cid",
]);
const POSTHOG_TEXT_MAX_LENGTH = 512;
const POSTHOG_ELEMENTS_CHAIN_MAX_LENGTH = 4096;
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const CONTAINS_EMAIL_PATTERN = /[^\s@/]+@[^\s@/]+\.[a-z]{2,}/i;
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

export function toAnalyticsToken(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
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

const SLUG_SEGMENT_PATTERN = /^[a-z0-9]{1,24}(?:-[a-z0-9]{1,24})+$/;
const HEX_TOKEN_PATTERN = /^[0-9a-f]{24,}$/i;

function isIdentifierPathSegment(decoded: string) {
  if (
    UUID_PATTERN.test(decoded) ||
    EMAIL_PATTERN.test(decoded) ||
    /^\d{6,}$/.test(decoded) ||
    HEX_TOKEN_PATTERN.test(decoded) ||
    decoded.length > 128
  ) {
    return true;
  }
  return decoded.length > 32 && !SLUG_SEGMENT_PATTERN.test(decoded);
}

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
      return isIdentifierPathSegment(decoded) ? ":id" : segment;
    })
    .join("/");
}

function normalizeSearch(search: string) {
  const params = new URLSearchParams(search);
  const kept = new URLSearchParams();
  for (const [key, value] of params) {
    if (
      SAFE_QUERY_PARAMETER_KEYS.has(key.toLowerCase()) &&
      value.length > 0 &&
      value.length <= 96 &&
      !CONTAINS_EMAIL_PATTERN.test(value)
    ) {
      kept.append(key, value);
    }
  }
  const serialized = kept.toString();
  return serialized ? `?${serialized}` : "";
}

function normalizePathProperty(value: string) {
  const pathname = normalizePath(value.split(/[?#]/, 1)[0]);
  return pathname.length <= 256 && SAFE_URL_PATTERN.test(pathname)
    ? pathname
    : undefined;
}

function normalizeUrl(value: string, origin: string, keepSearch: boolean) {
  let url: URL;
  try {
    url = new URL(value, origin);
  } catch {
    return undefined;
  }
  const location = `${url.origin}${normalizePath(url.pathname)}`;
  if (location.length > 256 || !SAFE_URL_PATTERN.test(location)) {
    return undefined;
  }
  return `${location}${keepSearch ? normalizeSearch(url.search) : ""}`;
}

function isPostHogTextValue(value: unknown, maxLength: number) {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= maxLength &&
    !CONTAINS_EMAIL_PATTERN.test(value)
  );
}

function sanitizePostHogScalar(value: unknown, maxLength: number): unknown {
  if (value === null || typeof value === "boolean") {
    return value;
  }
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : undefined;
  }
  if (typeof value === "string") {
    return isPostHogTextValue(value, maxLength) ? value : undefined;
  }
  if (Array.isArray(value)) {
    const sanitized = value
      .map((item) => sanitizePostHogScalar(item, maxLength))
      .filter((item) => item !== undefined);
    return sanitized.length === value.length ? sanitized : undefined;
  }
  return undefined;
}

function sanitizePostHogElements(value: unknown) {
  if (!Array.isArray(value)) return undefined;
  return value.flatMap((element) => {
    if (!element || typeof element !== "object") return [];
    const sanitized: Record<string, unknown> = {};
    for (const [key, raw] of Object.entries(element)) {
      if (!POSTHOG_ELEMENT_ATTRIBUTE_KEYS.has(key)) continue;
      const safe = sanitizePostHogScalar(raw, POSTHOG_TEXT_MAX_LENGTH);
      if (safe !== undefined) sanitized[key] = safe;
    }
    return [sanitized];
  });
}

function sanitizePostHogHeatmapData(value: unknown, origin: string) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const sanitized: Record<string, unknown> = {};
  for (const [pageUrl, entries] of Object.entries(value)) {
    const url = normalizeUrl(pageUrl, origin, true);
    if (url === undefined || !Array.isArray(entries)) continue;
    const safeEntries = entries.flatMap((entry) =>
      entry && typeof entry === "object" && !Array.isArray(entry)
        ? [sanitizeAnalyticsProperties(entry as Record<string, unknown>)]
        : [],
    );
    if (safeEntries.length > 0) {
      const previous = sanitized[url];
      sanitized[url] = Array.isArray(previous)
        ? [...previous, ...safeEntries]
        : safeEntries;
    }
  }
  return sanitized;
}

function isPostHogTextPropertyKey(key: string) {
  return (
    POSTHOG_TEXT_PROPERTY_KEYS.has(key) ||
    POSTHOG_INITIAL_TEXT_PROPERTY_KEYS.has(key) ||
    POSTHOG_TEXT_PROPERTY_PREFIXES.some((prefix) => key.startsWith(prefix))
  );
}

function isPostHogUrlPropertyKey(key: string) {
  return URL_PROPERTY_KEYS.has(key) && !POSTHOG_PATH_PROPERTY_KEYS.has(key);
}

function sanitizePostHogProperties(
  properties: Record<string, unknown>,
  origin: string,
): Record<string, unknown> {
  const sanitized: Record<string, unknown> = {};
  const custom: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(properties)) {
    if (isPostHogUrlPropertyKey(key)) {
      if (typeof value !== "string") continue;
      if (isReferrerPropertyKey(key) && value === "$direct") {
        sanitized[key] = value;
        continue;
      }
      const url = normalizeUrl(value, origin, !isReferrerPropertyKey(key));
      if (url !== undefined) sanitized[key] = url;
      continue;
    }
    if (POSTHOG_PATH_PROPERTY_KEYS.has(key)) {
      if (typeof value !== "string") continue;
      const pathname = normalizePathProperty(value);
      if (pathname !== undefined) sanitized[key] = pathname;
      continue;
    }
    if (key === "$initial_person_info") {
      if (value && typeof value === "object" && !Array.isArray(value)) {
        const info: Record<string, unknown> = {};
        const { r, u } = value as Record<string, unknown>;
        if (typeof r === "string") {
          const referrer = r === "$direct" ? r : normalizeUrl(r, origin, false);
          if (referrer !== undefined) info.r = referrer;
        }
        if (typeof u === "string") {
          const url = normalizeUrl(u, origin, true);
          if (url !== undefined) info.u = url;
        }
        sanitized[key] = info;
      }
      continue;
    }
    if (POSTHOG_NESTED_PROPERTY_KEYS.has(key)) {
      if (value && typeof value === "object" && !Array.isArray(value)) {
        sanitized[key] = sanitizePostHogProperties(
          value as Record<string, unknown>,
          origin,
        );
      }
      continue;
    }
    if (POSTHOG_ANONYMOUS_ID_KEYS.has(key)) {
      if (
        typeof value === "string" &&
        value.length <= 128 &&
        SAFE_TOKEN_PATTERN.test(value)
      ) {
        sanitized[key] = value;
      }
      continue;
    }
    if (key === "$elements") {
      const elements = sanitizePostHogElements(value);
      if (elements) sanitized[key] = elements;
      continue;
    }
    if (key === "$heatmap_data") {
      const heatmap = sanitizePostHogHeatmapData(value, origin);
      if (heatmap) sanitized[key] = heatmap;
      continue;
    }
    if (isPostHogTextPropertyKey(key)) {
      const safe = sanitizePostHogScalar(
        value,
        key === "$elements_chain"
          ? POSTHOG_ELEMENTS_CHAIN_MAX_LENGTH
          : POSTHOG_TEXT_MAX_LENGTH,
      );
      if (safe !== undefined) sanitized[key] = safe;
      continue;
    }
    custom[key] = value;
  }

  return { ...sanitized, ...sanitizeAnalyticsProperties(custom) };
}

export function sanitizePostHogEvent(
  event: CaptureResult | null,
  origin: string,
  projectToken: string,
) {
  if (!event) return null;
  return {
    ...event,
    event: sanitizeAnalyticsEventName(event.event),
    properties: {
      ...sanitizePostHogProperties({ ...event.properties }, origin),
      // PostHog authenticates capture with this public project key in the body.
      token: projectToken,
    },
  };
}
