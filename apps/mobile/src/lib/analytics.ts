import AsyncStorage from "@react-native-async-storage/async-storage";
import Constants from "expo-constants";
import { randomUUID } from "expo-crypto";
import { AppState } from "react-native";

import { env } from "@/lib/env";

type AnalyticsValue = null | boolean | number | string | string[] | number[];

const FIRST_OPEN_KEY = "anarlog:analytics:first-opened";
const DISTINCT_ID_KEY = "anarlog:analytics:distinct-id";
const SESSION_TIMEOUT_MS = 30 * 60 * 1_000;
const appVersion = Constants.expoConfig?.version ?? "unknown";
const enabled = Boolean(env.posthogApiKey) && !__DEV__;
const SENSITIVE_PROPERTY_KEYS = new Set([
  "account_id",
  "body",
  "content",
  "email",
  "error",
  "file_path",
  "full_name",
  "message",
  "name",
  "note_title",
  "path",
  "prompt",
  "query",
  "request",
  "response",
  "team_id",
  "text",
  "transcript",
  "url",
  "user_id",
]);
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const SAFE_EVENT_NAME_PATTERN = /^[a-z0-9_$.-]+$/i;

let anonymousIdPromise: Promise<string> | null = null;
let sessionId = randomUUID();
let lastEventAt = Date.now();
let lifecycleStarted = false;

function getAnonymousId() {
  anonymousIdPromise ??= (async () => {
    const stored = await AsyncStorage.getItem(DISTINCT_ID_KEY).catch(
      () => null,
    );
    if (stored) return stored;

    const created = randomUUID();
    await AsyncStorage.setItem(DISTINCT_ID_KEY, created).catch(() => {});
    return created;
  })();
  return anonymousIdPromise;
}

function currentSessionId() {
  const now = Date.now();
  if (now - lastEventAt >= SESSION_TIMEOUT_MS) {
    sessionId = randomUUID();
  }
  lastEventAt = now;
  return sessionId;
}

function sanitizeAnalyticsProperties(properties: Record<string, unknown>) {
  return Object.fromEntries(
    Object.entries(properties).filter(([key, value]) => {
      const normalizedKey = key.toLowerCase();
      return (
        !SENSITIVE_PROPERTY_KEYS.has(normalizedKey) &&
        !normalizedKey.endsWith("_email") &&
        !normalizedKey.endsWith("_id") &&
        !normalizedKey.endsWith("_path") &&
        !normalizedKey.endsWith("_url") &&
        isSafeAnalyticsValue(key, value)
      );
    }),
  );
}

function isSafeAnalyticsValue(key: string, value: unknown): boolean {
  if (value === null || typeof value === "boolean") return true;
  if (typeof value === "number") return Number.isFinite(value);
  if (typeof value === "string") {
    const allowsPath = key === "$screen_name";
    return (
      value.length > 0 &&
      value.length <= 96 &&
      !EMAIL_PATTERN.test(value) &&
      !UUID_PATTERN.test(value) &&
      (allowsPath
        ? /^[a-z0-9_.:{}<>/-]+$/i.test(value)
        : /^[a-z0-9_.:{}<>-]+$/i.test(value)) &&
      !(value.length >= 32 && /^[a-z0-9_-]+$/i.test(value))
    );
  }
  if (Array.isArray(value)) {
    return value.every((item) => isSafeAnalyticsValue(key, item));
  }
  return false;
}

function sanitizeAnalyticsEventName(event: string) {
  return event.length > 0 &&
    event.length <= 64 &&
    SAFE_EVENT_NAME_PATTERN.test(event)
    ? event
    : "analytics_event";
}

async function sendAnalytics(
  event: string,
  properties: Record<string, unknown> = {},
  distinctId?: string,
) {
  if (!enabled) return;

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 1_000);
  try {
    const resolvedDistinctId = distinctId ?? (await getAnonymousId());
    await fetch(`${env.posthogHost.replace(/\/+$/, "")}/capture/`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      signal: controller.signal,
      body: JSON.stringify({
        api_key: env.posthogApiKey,
        event: sanitizeAnalyticsEventName(event),
        properties: {
          ...sanitizeAnalyticsProperties(properties),
          distinct_id: resolvedDistinctId,
          $session_id: currentSessionId(),
          surface: "mobile",
          build_variant: env.appVariant,
          analytics_schema_version: 1,
          app_version: appVersion,
        },
      }),
    });
  } catch {
  } finally {
    clearTimeout(timeout);
  }
}

export function captureAnalytics(
  event: string,
  properties: Record<string, AnalyticsValue> = {},
) {
  void sendAnalytics(event, properties);
}

export function screenAnalytics(pathname: string) {
  captureAnalytics("$screen", {
    $screen_name: pathname
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
      .join("/"),
  });
}

export function identifyAnalytics(_userId: string) {
  // Mobile analytics intentionally remain install-scoped and anonymous.
}

export function resetAnalytics() {
  sessionId = randomUUID();
  lastEventAt = Date.now();
  const nextAnonymousId = randomUUID();
  anonymousIdPromise = Promise.resolve(nextAnonymousId);
  void AsyncStorage.setItem(DISTINCT_ID_KEY, nextAnonymousId).catch(() => {});
}

let initialization: Promise<void> | null = null;

export function initializeAnalytics() {
  initialization ??= (async () => {
    if (!enabled) return;

    await getAnonymousId();
    await sendAnalytics("app_started");
    await sendAnalytics("$app_opened");

    const firstOpen = await AsyncStorage.getItem(FIRST_OPEN_KEY).catch(
      () => null,
    );
    if (firstOpen === null) {
      await AsyncStorage.setItem(FIRST_OPEN_KEY, "1").catch(() => {});
      await sendAnalytics("app_first_opened", {
        first_open_marker: "local_install",
      });
    }

    if (lifecycleStarted) return;
    lifecycleStarted = true;
    let previousState = AppState.currentState;
    AppState.addEventListener("change", (nextState) => {
      if (nextState === previousState) return;
      if (nextState === "active") {
        captureAnalytics("$app_opened");
      } else if (previousState === "active") {
        captureAnalytics("$app_backgrounded");
      }
      previousState = nextState;
    });
  })();
  return initialization;
}
