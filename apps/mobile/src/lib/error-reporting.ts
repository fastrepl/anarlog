import * as Sentry from "@sentry/react-native";
import type {
  Breadcrumb,
  ErrorEvent,
  SeverityLevel,
} from "@sentry/react-native";
import Constants from "expo-constants";
import { Platform } from "react-native";

import { env } from "@/lib/env";
import {
  normalizeOperationalError,
  operationalErrorMetadata,
  sanitizeBreadcrumb,
  sanitizeMobileErrorEvent,
  type MobileErrorEvent,
} from "@/lib/error-sanitization";

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

export function initializeErrorReporting() {
  Sentry.init({
    dsn: env.sentryDsn,
    enabled: Boolean(env.sentryDsn) && !__DEV__,
    environment: env.appVariant === "stable" ? "production" : env.appVariant,
    release: `anarlog-mobile@${Constants.expoConfig?.version ?? "unknown"}`,
    dist: appDist(),
    sendDefaultPii: false,
    attachStacktrace: true,
    beforeSend: sanitizeErrorEvent,
    beforeBreadcrumb: (breadcrumb) =>
      sanitizeBreadcrumb(breadcrumb as Breadcrumb),
    enableAutoSessionTracking: true,
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

export function setErrorReportingUser(userId: string | null) {
  Sentry.setUser(userId ? { id: userId } : null);
}
