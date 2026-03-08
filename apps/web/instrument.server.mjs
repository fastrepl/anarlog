import * as Sentry from "@sentry/tanstackstart-react";

Sentry.init({
  dsn: process.env.SENTRY_DSN,
  sendDefaultPii: true,
});
