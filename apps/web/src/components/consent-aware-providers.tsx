import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { useMountEffect } from "@/hooks/useMountEffect";
import { PostHogProvider } from "@/providers/posthog";

import { usePrivacyConsent } from "./privacy-consent";

const GOOGLE_TAG_ID = "google-tag";
const GOOGLE_ANALYTICS_ID = "G-4CDGPKJ8JB";
const MICROSOFT_CLARITY_SCRIPT_ID = "microsoft-clarity-script";
const MICROSOFT_CLARITY_TAG_ID = "wcjttoibok";

type ClarityFunction = ((...args: unknown[]) => void) & {
  q?: IArguments[];
};
type ClarityWindow = Window &
  typeof globalThis & {
    clarity?: ClarityFunction;
  };

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

function ensureMicrosoftClarityScript() {
  if (document.getElementById(MICROSOFT_CLARITY_SCRIPT_ID)) {
    return;
  }

  const clarityWindow = window as ClarityWindow;
  clarityWindow.clarity =
    clarityWindow.clarity ??
    function clarity() {
      const queuedClarity = clarityWindow.clarity;
      if (!queuedClarity) {
        return;
      }

      queuedClarity.q = queuedClarity.q ?? [];
      queuedClarity.q.push(arguments);
    };

  const script = document.createElement("script");
  script.id = MICROSOFT_CLARITY_SCRIPT_ID;
  script.async = true;
  script.src = `https://www.clarity.ms/tag/${MICROSOFT_CLARITY_TAG_ID}`;
  document.head.appendChild(script);
}

function MicrosoftClarityConsent({
  enabled,
  isReady,
}: {
  enabled: boolean;
  isReady: boolean;
}) {
  useMountEffect(() => {
    if (
      !isReady ||
      typeof window === "undefined" ||
      import.meta.env.DEV ||
      window.location.pathname.startsWith("/admin")
    ) {
      return;
    }

    if (enabled) {
      ensureMicrosoftClarityScript();
    }

    const clarity = (window as ClarityWindow).clarity;
    if (!clarity) {
      return;
    }

    clarity("consentv2", {
      ad_Storage: "denied",
      analytics_Storage: enabled ? "granted" : "denied",
    });

    if (!enabled) {
      clarity("consent", false);
    }
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
  const { analyticsEnabled, isReady } = usePrivacyConsent();

  return (
    <PostHogProvider enabled={analyticsEnabled}>
      <QueryClientProvider client={queryClient}>
        {children}
        <MicrosoftClarityConsent
          key={`${isReady}:${analyticsEnabled}`}
          enabled={analyticsEnabled}
          isReady={isReady}
        />
        {analyticsEnabled ? <GoogleAnalyticsScript /> : null}
      </QueryClientProvider>
    </PostHogProvider>
  );
}
