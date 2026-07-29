import * as Sentry from "@sentry/react";
import type { ErrorEvent, SeverityLevel } from "@sentry/react";

import { env } from "./env";

type ErrorContextValue = null | boolean | number | string;
const SAFE_IDENTIFIER_RE = /^[a-zA-Z0-9_.:/-]{1,128}$/;

function safeIdentifier(value: unknown): string | undefined {
  return typeof value === "string" && SAFE_IDENTIFIER_RE.test(value)
    ? value
    : undefined;
}

export function operationalErrorMetadata(error: unknown) {
  if (!error || typeof error !== "object") {
    return { type: "Error" };
  }

  const record = error as Record<string, unknown>;
  const statusValue = record.status ?? record.statusCode;
  const status =
    typeof statusValue === "number" &&
    Number.isInteger(statusValue) &&
    statusValue >= 100 &&
    statusValue <= 599
      ? statusValue
      : undefined;

  return {
    type: safeIdentifier(record.name) ?? "Error",
    code: safeIdentifier(record.code),
    stage: safeIdentifier(record.stage),
    status,
  };
}

export function normalizeOperationalError(
  error: unknown,
  operation: string,
): Error {
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
  delete event.message;
  delete event.logentry;
  if (event.exception?.values) {
    event.exception.values = event.exception.values.map((exception) => {
      const sanitized = {
        ...exception,
        value: exception.type ? `${exception.type} captured` : "Error captured",
      };
      if (sanitized.mechanism) {
        delete sanitized.mechanism.data;
      }
      return sanitized;
    });
  }
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
  if (!env.VITE_SENTRY_DSN) return;

  Sentry.init({
    dsn: env.VITE_SENTRY_DSN,
    release: env.VITE_APP_VERSION
      ? `anarlog-desktop@${env.VITE_APP_VERSION}`
      : undefined,
    environment: import.meta.env.MODE,
    sendDefaultPii: false,
    tracePropagationTargets: [],
    beforeSend: sanitizeErrorEvent,
    integrations: [
      Sentry.replayIntegration({
        blockAllMedia: true,
        maskAllText: true,
      }),
    ],
    replaysSessionSampleRate: 0.1,
    replaysOnErrorSampleRate: 1.0,
    initialScope: {
      tags: {
        "service.name": "desktop",
        "service.namespace": "anarlog",
        "anarlog.surface": "desktop",
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

export function setErrorReportingUser(userId: string | null) {
  Sentry.setUser(userId ? { id: userId } : null);
}
