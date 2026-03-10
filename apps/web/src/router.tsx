import { OutlitProvider } from "@outlit/browser/react";
import * as Sentry from "@sentry/tanstackstart-react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import { setupRouterSsrQueryIntegration } from "@tanstack/react-router-ssr-query";

import { env } from "./env";
import { PostHogProvider } from "./providers/posthog";
import { routeTree } from "./routeTree.gen";

function MaybeOutlitProvider({ children }: { children: React.ReactNode }) {
  if (env.VITE_OUTLIT_PUBLIC_KEY) {
    return (
      <OutlitProvider publicKey={env.VITE_OUTLIT_PUBLIC_KEY} trackPageviews>
        {children}
      </OutlitProvider>
    );
  }
  return <>{children}</>;
}

export function getRouter() {
  const queryClient = new QueryClient();

  const router = createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: "intent",
    scrollRestoration: true,
    trailingSlash: "always",
    Wrap: (props: { children: React.ReactNode }) => {
      return (
        <PostHogProvider>
          <MaybeOutlitProvider>
            <QueryClientProvider client={queryClient}>
              {props.children}
            </QueryClientProvider>
          </MaybeOutlitProvider>
        </PostHogProvider>
      );
    },
  });

  if (!router.isServer && env.VITE_SENTRY_DSN) {
    Sentry.init({
      dsn: env.VITE_SENTRY_DSN,
      release: env.VITE_APP_VERSION
        ? `hyprnote-web@${env.VITE_APP_VERSION}`
        : undefined,
      sendDefaultPii: true,
      integrations: [Sentry.replayIntegration()],
      replaysSessionSampleRate: 0.1,
      replaysOnErrorSampleRate: 1.0,
      tracePropagationTargets: [],
    });
  }

  setupRouterSsrQueryIntegration({ router, queryClient });

  return router;
}
