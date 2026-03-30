import * as Sentry from "@sentry/tanstackstart-react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import { setupRouterSsrQueryIntegration } from "@tanstack/react-router-ssr-query";
import { useEffect } from "react";

import {
  PrivacyConsentProvider,
  usePrivacyConsent,
} from "./components/privacy-consent";
import { env } from "./env";
import { PostHogProvider } from "./providers/posthog";
import { routeTree } from "./routeTree.gen";

const ZENDESK_SNIPPET_ID = "ze-snippet";
const ZENDESK_SNIPPET_SRC =
  "https://static.zdassets.com/ekr/snippet.js?key=15949e47-ed5a-4e52-846e-200dd0b8f4b9";

function MaybeZendeskWidget({ enabled }: { enabled: boolean }) {
  useEffect(() => {
    if (
      typeof document === "undefined" ||
      import.meta.env.DEV ||
      !enabled ||
      window.location.pathname.startsWith("/admin")
    ) {
      return;
    }

    if (document.getElementById(ZENDESK_SNIPPET_ID)) {
      return;
    }

    const script = document.createElement("script");
    script.id = ZENDESK_SNIPPET_ID;
    script.src = ZENDESK_SNIPPET_SRC;
    script.async = true;
    document.body.appendChild(script);
  }, [enabled]);

  return null;
}

function ConsentAwareProviders({
  children,
  queryClient,
}: {
  children: React.ReactNode;
  queryClient: QueryClient;
}) {
  const { analyticsEnabled } = usePrivacyConsent();

  return (
    <PostHogProvider enabled={analyticsEnabled}>
      <QueryClientProvider client={queryClient}>
        {children}
        <MaybeZendeskWidget enabled={analyticsEnabled} />
      </QueryClientProvider>
    </PostHogProvider>
  );
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
        <PrivacyConsentProvider>
          <ConsentAwareProviders queryClient={queryClient}>
            {props.children}
          </ConsentAwareProviders>
        </PrivacyConsentProvider>
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
      tracePropagationTargets: [],
    });
  }

  setupRouterSsrQueryIntegration({ router, queryClient });

  return router;
}
