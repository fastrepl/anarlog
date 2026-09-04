import * as Sentry from "@sentry/tanstackstart-react";

import { isUserErrorEvent } from "@anlg/user-error";

function sanitizeUrl(value) {
  return value?.split(/[?#]/, 1)[0];
}

Sentry.init({
  dsn: process.env.SENTRY_DSN,
  environment: process.env.NODE_ENV ?? "production",
  release: process.env.APP_VERSION
    ? `anarlog-web@${process.env.APP_VERSION}`
    : undefined,
  sendDefaultPii: false,
  beforeSend(event) {
    if (isUserErrorEvent(event)) return null;

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
          value: exception.type
            ? `${exception.type} captured`
            : "Error captured",
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
  },
  initialScope: {
    tags: {
      "service.name": "web",
      "service.namespace": "anarlog",
      "anarlog.surface": "web_server",
    },
  },
});
