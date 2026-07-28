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

let anonymousIdPromise: Promise<string> | null = null;
let identifiedUserId: string | null = null;
let accountId: string | null = null;
let identityGeneration = 0;
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

async function sendAnalytics(
  event: string,
  properties: Record<string, unknown> = {},
  distinctId?: string,
) {
  if (!enabled) return;

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 1_000);
  try {
    const resolvedDistinctId =
      distinctId ?? identifiedUserId ?? (await getAnonymousId());
    await fetch(`${env.posthogHost.replace(/\/+$/, "")}/capture/`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      signal: controller.signal,
      body: JSON.stringify({
        api_key: env.posthogApiKey,
        event,
        properties: {
          ...properties,
          distinct_id: resolvedDistinctId,
          $session_id: currentSessionId(),
          ...(accountId ? { $groups: { account: accountId } } : {}),
          surface: "mobile",
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
    $screen_name: pathname,
  });
}

export function identifyAnalytics(userId: string) {
  if (identifiedUserId === userId) return;

  const generation = ++identityGeneration;
  identifiedUserId = userId;
  accountId = userId;
  void getAnonymousId().then((resolvedAnonymousId) => {
    if (generation !== identityGeneration || identifiedUserId !== userId) {
      return;
    }

    void sendAnalytics(
      "$identify",
      { $anon_distinct_id: resolvedAnonymousId },
      userId,
    );
    void sendAnalytics(
      "$groupidentify",
      {
        $group_type: "account",
        $group_key: userId,
        $group_set: {},
      },
      userId,
    );
  });
}

export function resetAnalytics() {
  if (!identifiedUserId && !accountId) {
    return;
  }

  identityGeneration += 1;
  identifiedUserId = null;
  accountId = null;
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
