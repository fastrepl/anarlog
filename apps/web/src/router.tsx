import * as Sentry from "@sentry/tanstackstart-react";
import { QueryClient } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import { setupRouterSsrQueryIntegration } from "@tanstack/react-router-ssr-query";

import { env } from "./env";
import {
  isShareRoutePathname,
  prepareShareRoutePrivacy,
} from "./lib/share-route-privacy";
import { routeTree } from "./routeTree.gen";

export function getRouter() {
  const queryClient = new QueryClient();

  const router = createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: "intent",
    scrollRestoration: true,
    trailingSlash: "always",
  });

  prepareShareRoutePrivacy();

  if (
    !router.isServer &&
    env.VITE_SENTRY_DSN &&
    !isShareRoutePathname(window.location.pathname)
  ) {
    Sentry.init({
      dsn: env.VITE_SENTRY_DSN,
      release: env.VITE_APP_VERSION
        ? `anarlog-web@${env.VITE_APP_VERSION}`
        : undefined,
      sendDefaultPii: true,
      tracePropagationTargets: [],
      beforeSend: (event) =>
        isShareRoutePathname(window.location.pathname) ? null : event,
      beforeSendTransaction: (event) =>
        isShareRoutePathname(window.location.pathname) ? null : event,
    });
  }

  setupRouterSsrQueryIntegration({ router, queryClient });

  return router;
}
