import type {
  Breadcrumb,
  ErrorEvent,
  SeverityLevel,
} from "@sentry/react-native";
import * as Sentry from "@sentry/react-native";
import Constants from "expo-constants";
import { Platform } from "react-native";

import { isUserError, isUserErrorEvent } from "@anlg/user-error";

import { env } from "@/lib/env";
import {
  normalizeOperationalError,
  operationalErrorMetadata,
  sanitizeBreadcrumb,
  sanitizeMobileErrorEvent,
  type MobileErrorEvent,
} from "@/lib/error-sanitization";
import {
  getPrivacyPreferences,
  loadPrivacyPreferences,
  subscribePrivacyPreferences,
} from "@/settings/privacy-store";

type ErrorContextValue = null | boolean | number | string;
const capturedErrors = new WeakSet<object>();

export function sanitizeErrorEvent(event: ErrorEvent): ErrorEvent {
  return sanitizeMobileErrorEvent(
    event as unknown as MobileErrorEvent,
  ) as unknown as ErrorEvent;
}

function appDist(): string | undefined {
  const build =
    Platform.OS === "ios"
      ? (Constants.platform?.ios?.buildNumber ??
        Constants.expoConfig?.ios?.buildNumber)
      : (Constants.platform?.android?.versionCode ??
        Constants.expoConfig?.android?.versionCode);
  return build === undefined ? undefined : String(build);
}

function startErrorReporting() {
  Sentry.init({
    dsn: env.sentryDsn,
    enabled: true,
    environment: env.appVariant === "stable" ? "production" : env.appVariant,
    release: `anarlog-mobile@${Constants.expoConfig?.version ?? "unknown"}`,
    dist: appDist(),
    sendDefaultPii: false,
    attachStacktrace: true,
    beforeSend: (event) =>
      !getPrivacyPreferences().errorReports || isUserErrorEvent(event)
        ? null
        : sanitizeErrorEvent(event),
    beforeBreadcrumb: (breadcrumb) =>
      sanitizeBreadcrumb(breadcrumb as Breadcrumb),
    enableAutoSessionTracking: false,
    sessionTrackingIntervalMillis: 30_000,
    enableTombstone: true,
    enableAppHangTracking: true,
    appHangTimeoutInterval: 5,
    enableWatchdogTerminationTracking: true,
    attachScreenshot: false,
    attachViewHierarchy: false,
    initialScope: {
      tags: {
        "service.name": "mobile",
        "service.namespace": "anarlog",
        "anarlog.mobile.app_variant": env.appVariant,
        "anarlog.mobile.execution_environment": Constants.executionEnvironment,
        "anarlog.mobile.os": Platform.OS,
        "anarlog.surface": "mobile",
      },
    },
  });
}

let initialized = false;
export async function initializeErrorReporting() {
  await loadPrivacyPreferences();
  if (initialized || !env.sentryDsn || __DEV__) return;
  initialized = true;
  let enabled = getPrivacyPreferences().errorReports;
  let changing = Promise.resolve();
  if (enabled) startErrorReporting();
  subscribePrivacyPreferences(() => {
    const next = getPrivacyPreferences().errorReports;
    if (next === enabled) return;
    enabled = next;
    const client = Sentry.getClient();
    if (client) client.getOptions().enabled = false;
    changing = changing
      .catch(() => {})
      .then(async () => {
        await Sentry.close();
        if (getPrivacyPreferences().errorReports) startErrorReporting();
      });
  });
}

export function captureOperationalError(
  error: unknown,
  {
    operation,
    level = "error",
    tags,
    context,
  }: {
    operation: string;
    level?: SeverityLevel;
    tags?: Record<string, ErrorContextValue>;
    context?: Record<string, ErrorContextValue>;
  },
) {
  if (!getPrivacyPreferences().errorReports) return;
  if (isUserError(error)) return;

  if (error && typeof error === "object") {
    if (capturedErrors.has(error)) return;
    capturedErrors.add(error);
  }

  const metadata = operationalErrorMetadata(error);
  const exception = normalizeOperationalError(error, operation);

  return Sentry.withScope((scope) => {
    scope.setLevel(level);
    scope.setTag("anarlog.operation", operation);
    scope.setTag("error.type", metadata.type);
    if (metadata.code) scope.setTag("error.code", metadata.code);
    if (metadata.stage) {
      scope.setTag("anarlog.error.stage", metadata.stage);
    }
    if (metadata.status) {
      scope.setTag("http.response.status_code", metadata.status);
    }
    for (const [key, value] of Object.entries(tags ?? {})) {
      if (value !== null) {
        scope.setTag(`anarlog.${key}`, value);
      }
    }
    if (context) {
      scope.setContext("anarlog.operation", context);
    }
    return Sentry.captureException(exception);
  });
}

export function addOperationalBreadcrumb(
  operation: string,
  data?: Record<string, ErrorContextValue>,
) {
  Sentry.addBreadcrumb({
    category: "anarlog.operation",
    level: "info",
    message: operation,
    data,
  });
}

export function addNavigationBreadcrumb(from: string | null, to: string) {
  Sentry.addBreadcrumb({
    category: "navigation",
    level: "info",
    data: { from: from ?? "", to },
  });
}

export function setErrorReportingUser(_userId: string | null) {
  Sentry.setUser(null);
}
