import * as Sentry from "@sentry/tanstackstart-react";
import { QueryClient } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import { setupRouterSsrQueryIntegration } from "@tanstack/react-router-ssr-query";

import { ErrorPage } from "./components/error-page";
import { env } from "./env";
import { isTelemetryPrivateLocation } from "./lib/auth-route-privacy";
import { sanitizeErrorEvent } from "./lib/error-reporting";
import { prepareShareRoutePrivacy } from "./lib/share-route-privacy";
import { routeTree } from "./routeTree.gen";

export function getRouter() {
  const queryClient = new QueryClient();

  const router = createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: "intent",
    defaultErrorComponent: ErrorPage,
    scrollRestoration: true,
    trailingSlash: "always",
  });

  prepareShareRoutePrivacy();

  if (!router.isServer && env.VITE_SENTRY_DSN) {
    Sentry.init({
      dsn: env.VITE_SENTRY_DSN,
      release: env.VITE_APP_VERSION
        ? `anarlog-web@${env.VITE_APP_VERSION}`
        : undefined,
      sendDefaultPii: false,
      tracePropagationTargets: [],
      beforeSend: (event) =>
        isTelemetryPrivateLocation(
          window.location.pathname,
          window.location.search,
        )
          ? null
          : sanitizeErrorEvent(event),
      beforeSendTransaction: (event) =>
        isTelemetryPrivateLocation(
          window.location.pathname,
          window.location.search,
        )
          ? null
          : event,
      initialScope: {
        tags: {
          "service.name": "web",
          "service.namespace": "anarlog",
          "anarlog.surface": "web",
        },
      },
    });
  }

  setupRouterSsrQueryIntegration({ router, queryClient });

  return router;
}
