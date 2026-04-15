import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { useMountEffect } from "@/hooks/useMountEffect";
import { PostHogProvider } from "@/providers/posthog";

import { usePrivacyConsent } from "./privacy-consent";

const CHATWOOT_SNIPPET_ID = "chatwoot-snippet";
const CHATWOOT_BASE_URL = "https://app.chatwoot.com";
const CHATWOOT_WEBSITE_TOKEN = "FH1mNsxrXPZLcgi3Rfgrb13R";
const GOOGLE_TAG_ID = "google-tag";
const GOOGLE_ANALYTICS_ID = "G-4CDGPKJ8JB";

type AnalyticsWindow = Window &
  typeof globalThis & {
    dataLayer?: unknown[];
    gtag?: (...args: unknown[]) => void;
  };

function GoogleAnalyticsScript() {
  useMountEffect(() => {
    if (
      typeof document === "undefined" ||
      import.meta.env.DEV ||
      window.location.pathname.startsWith("/admin")
    ) {
      return;
    }

    if (document.getElementById(GOOGLE_TAG_ID)) {
      return;
    }

    const analyticsWindow = window as AnalyticsWindow;
    analyticsWindow.dataLayer = analyticsWindow.dataLayer ?? [];
    analyticsWindow.gtag =
      analyticsWindow.gtag ??
      function gtag() {
        analyticsWindow.dataLayer?.push(arguments);
      };
    analyticsWindow.gtag("js", new Date());
    analyticsWindow.gtag("config", GOOGLE_ANALYTICS_ID);

    const script = document.createElement("script");
    script.id = GOOGLE_TAG_ID;
    script.src = `https://www.googletagmanager.com/gtag/js?id=${GOOGLE_ANALYTICS_ID}`;
    script.async = true;
    document.head.appendChild(script);
  });

  return null;
}

type ChatwootWindow = Window &
  typeof globalThis & {
    chatwootSDK?: {
      run: (config: { websiteToken: string; baseUrl: string }) => void;
    };
  };

function ChatwootWidget() {
  useMountEffect(() => {
    if (
      typeof document === "undefined" ||
      import.meta.env.DEV ||
      window.location.pathname.startsWith("/admin")
    ) {
      return;
    }

    if (document.getElementById(CHATWOOT_SNIPPET_ID)) {
      return;
    }

    const script = document.createElement("script");
    script.id = CHATWOOT_SNIPPET_ID;
    script.src = `${CHATWOOT_BASE_URL}/packs/js/sdk.js`;
    script.async = true;
    script.onload = () => {
      (window as ChatwootWindow).chatwootSDK?.run({
        websiteToken: CHATWOOT_WEBSITE_TOKEN,
        baseUrl: CHATWOOT_BASE_URL,
      });
    };
    document.body.appendChild(script);
  });

  return null;
}

export function ConsentAwareProviders({
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
        {analyticsEnabled ? <GoogleAnalyticsScript /> : null}
        {analyticsEnabled ? <ChatwootWidget /> : null}
      </QueryClientProvider>
    </PostHogProvider>
  );
}
