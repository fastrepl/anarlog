import * as Sentry from "@sentry/tanstackstart-react";
import type { ErrorEvent, SeverityLevel } from "@sentry/tanstackstart-react";

import { isUserError, isUserErrorEvent } from "@anlg/user-error";

type ErrorContextValue = null | boolean | number | string;
const SAFE_IDENTIFIER_RE = /^[a-zA-Z0-9_.:/-]{1,128}$/;
const SAFE_STACK_FUNCTION_RE = /^[a-zA-Z0-9_$.[\]<>-]{1,128}$/;

function safeIdentifier(value: unknown): string | undefined {
  return typeof value === "string" && SAFE_IDENTIFIER_RE.test(value)
    ? value
    : undefined;
}

function safeStackFunction(value: unknown): string | undefined {
  return typeof value === "string" && SAFE_STACK_FUNCTION_RE.test(value)
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

function normalizeOperationalError(error: unknown, operation: string): Error {
  const normalized = new Error(`${operation} failed`);
  const metadata = operationalErrorMetadata(error);
  normalized.name = metadata.type;

  return normalized;
}

const SAFE_TAGS = new Set([
  "anarlog.error.stage",
  "anarlog.operation",
  "anarlog.surface",
  "error.code",
  "error.type",
  "http.response.status_code",
  "service.name",
  "service.namespace",
]);

function sanitizeTags(tags: ErrorEvent["tags"]) {
  if (!tags) return undefined;
  const safe = Object.fromEntries(
    Object.entries(tags).filter(
      ([key, value]) =>
        SAFE_TAGS.has(key) &&
        (typeof value === "number" || safeIdentifier(value) !== undefined),
    ),
  );
  return Object.keys(safe).length > 0 ? safe : undefined;
}

type ErrorException = NonNullable<
  NonNullable<ErrorEvent["exception"]>["values"]
>[number];
type ErrorStacktrace = NonNullable<ErrorException["stacktrace"]>;

function sanitizeStacktrace(stacktrace: ErrorStacktrace) {
  stacktrace.frames = stacktrace.frames?.map((frame) => ({
    colno: frame.colno,
    filename: "source",
    function: safeStackFunction(frame.function),
    in_app: frame.in_app,
    lineno: frame.lineno,
  }));
  return stacktrace;
}

export function sanitizeErrorEvent(event: ErrorEvent): ErrorEvent {
  delete event.user;
  delete event.request;
  delete event.contexts;
  delete event.extra;
  delete event.message;
  delete event.logentry;
  delete event.transaction;
  event.tags = sanitizeTags(event.tags);
  if (event.exception?.values) {
    event.exception.values = event.exception.values.map((exception) => {
      const sanitized = {
        ...exception,
        value: exception.type ? `${exception.type} captured` : "Error captured",
      };
      delete sanitized.module;
      if (sanitized.stacktrace) {
        sanitized.stacktrace = sanitizeStacktrace(sanitized.stacktrace);
      }
      if (sanitized.mechanism) {
        const original = sanitized.mechanism;
        sanitized.mechanism = {
          type: safeIdentifier(original.type) ?? "generic",
          ...(original.handled === undefined
            ? {}
            : { handled: original.handled }),
          ...(original.synthetic === undefined
            ? {}
            : { synthetic: original.synthetic }),
        };
      }
      return sanitized;
    });
  }
  event.breadcrumbs = event.breadcrumbs?.map((breadcrumb) => ({
    category: breadcrumb.category,
    level: breadcrumb.level,
    timestamp: breadcrumb.timestamp,
    type: breadcrumb.type,
  }));

  return event;
}

function isStacklessUnhandledRejection(event: ErrorEvent) {
  return event.exception?.values?.some((exception) => {
    const isUnhandledRejection =
      exception.type === "UnhandledRejection" ||
      exception.mechanism?.type.endsWith("onunhandledrejection");
    return isUnhandledRejection && !exception.stacktrace?.frames?.length;
  });
}

export function createErrorEventFilter() {
  let reportedStacklessUnhandledRejection = false;

  return (event: ErrorEvent): ErrorEvent | null => {
    if (isUserErrorEvent(event)) return null;

    const sanitized = sanitizeErrorEvent(event);
    if (!isStacklessUnhandledRejection(sanitized)) return sanitized;
    if (reportedStacklessUnhandledRejection) return null;

    reportedStacklessUnhandledRejection = true;
    sanitized.fingerprint = ["web", "stackless-unhandled-rejection"];
    sanitized.tags = {
      ...sanitized.tags,
      "error.type": "stackless_unhandled_rejection",
    };
    return sanitized;
  };
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
  if (isUserError(error)) return;

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
    scope.setTag(
      "anarlog.surface",
      typeof window === "undefined" ? "web_server" : "web",
    );
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

export function setErrorReportingUser(_userId: string | null) {
  Sentry.setUser(null);
}
