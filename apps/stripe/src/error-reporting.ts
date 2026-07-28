import * as Sentry from "@sentry/bun";
import type { SeverityLevel } from "@sentry/bun";

type ErrorContextValue = null | boolean | number | string;

function sanitizeUrl(value: string | undefined) {
  return value?.split(/[?#]/, 1)[0];
}

export function sanitizeErrorEvent(
  event: Sentry.ErrorEvent,
): Sentry.ErrorEvent {
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
  const exception =
    error instanceof Error ? error : new Error(`${operation} failed`);

  return Sentry.withScope((scope) => {
    scope.setLevel(level);
    scope.setTag("hyprnote.operation", operation);
    scope.setTag("hyprnote.surface", "billing");
    for (const [key, value] of Object.entries(tags ?? {})) {
      if (value !== null) {
        scope.setTag(`hyprnote.${key}`, value);
      }
    }
    if (context) {
      scope.setContext("hyprnote.operation", context);
    }
    return Sentry.captureException(exception);
  });
}
