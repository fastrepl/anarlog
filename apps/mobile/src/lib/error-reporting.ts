import * as Sentry from "@sentry/react-native";
import type { ErrorEvent, SeverityLevel } from "@sentry/react-native";
import Constants from "expo-constants";

import { env } from "@/lib/env";

type ErrorContextValue = null | boolean | number | string;

function normalizeOperationalError(error: unknown, operation: string): Error {
  if (error instanceof Error) return error;

  if (
    typeof error === "string" ||
    typeof error === "number" ||
    typeof error === "boolean"
  ) {
    return new Error(`${operation} failed: ${String(error).slice(0, 256)}`);
  }

  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    const details = ["status", "statusCode", "code", "message"]
      .flatMap((key) => {
        const value = record[key];
        return typeof value === "string" ||
          typeof value === "number" ||
          typeof value === "boolean"
          ? [`${key}=${String(value).slice(0, 256)}`]
          : [];
      })
      .join(", ");

    if (details) {
      return new Error(`${operation} failed (${details})`);
    }
  }

  return new Error(`${operation} failed`);
}

function sanitizeUrl(value: string | undefined) {
  if (!value) return value;

  try {
    const url = new URL(value);
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return value.split(/[?#]/, 1)[0];
  }
}

export function sanitizeErrorEvent(event: ErrorEvent): ErrorEvent {
  if (event.user) {
    event.user = event.user.id ? { id: event.user.id } : undefined;
  }

  if (event.request) {
    event.request = {
      method: event.request.method,
      url: sanitizeUrl(event.request.url),
    };
  }

  delete event.extra;
  event.breadcrumbs = event.breadcrumbs?.map((breadcrumb) => ({
    category: breadcrumb.category,
    level: breadcrumb.level,
    timestamp: breadcrumb.timestamp,
    type: breadcrumb.type,
    ...(breadcrumb.category === "navigation"
      ? {
          data: {
            from: sanitizeUrl(String(breadcrumb.data?.from ?? "")),
            to: sanitizeUrl(String(breadcrumb.data?.to ?? "")),
          },
        }
      : {}),
  }));

  return event;
}

export function initializeErrorReporting() {
  Sentry.init({
    dsn: env.sentryDsn,
    enabled: Boolean(env.sentryDsn) && !__DEV__,
    environment: __DEV__ ? "development" : "production",
    release: `anarlog-mobile@${Constants.expoConfig?.version ?? "unknown"}`,
    sendDefaultPii: false,
    attachStacktrace: true,
    beforeSend: sanitizeErrorEvent,
    initialScope: {
      tags: {
        "service.name": "mobile",
        "service.namespace": "hyprnote",
        surface: "mobile",
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
  const exception = normalizeOperationalError(error, operation);

  return Sentry.withScope((scope) => {
    scope.setLevel(level);
    scope.setTag("operation", operation);
    for (const [key, value] of Object.entries(tags ?? {})) {
      if (value !== null) {
        scope.setTag(key, value);
      }
    }
    if (context) {
      scope.setContext("operation", context);
    }
    return Sentry.captureException(exception);
  });
}

export function setErrorReportingUser(userId: string | null) {
  Sentry.setUser(userId ? { id: userId } : null);
}
