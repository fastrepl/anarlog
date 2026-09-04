import * as Sentry from "@sentry/tanstackstart-react";

import { isUserErrorEvent } from "@anlg/user-error";

import { sanitizeServerErrorEvent } from "./instrument.server-sanitization.mjs";

Sentry.init({
  dsn: process.env.SENTRY_DSN,
  environment: process.env.NODE_ENV ?? "production",
  release: process.env.APP_VERSION
    ? `anarlog-web@${process.env.APP_VERSION}`
    : undefined,
  sendDefaultPii: false,
  beforeSend(event) {
    if (isUserErrorEvent(event)) return null;
    return sanitizeServerErrorEvent(event);
  },
  initialScope: {
    tags: {
      "service.name": "web",
      "service.namespace": "anarlog",
      "anarlog.surface": "web_server",
    },
  },
});
