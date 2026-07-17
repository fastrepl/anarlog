import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { useMountEffect } from "@/hooks/useMountEffect";
import { isShareRoutePathname } from "@/lib/share-route-privacy";
import { PostHogProvider } from "@/providers/posthog";
import { bootstrapBrowserTelemetry, stopBrowserTelemetry } from "@/telemetry";

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
      window.location.pathname.startsWith("/admin") ||
      isShareRoutePathname(window.location.pathname)
    ) {
      return;
    }

    setGoogleAnalyticsDisabled(false);

    if (document.getElementById(GOOGLE_TAG_ID)) {
      return () => setGoogleAnalyticsDisabled(true);
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

    return () => setGoogleAnalyticsDisabled(true);
  });

  return null;
}

function MicrosoftClarityScript() {
  useMountEffect(() => {
    if (
      typeof document === "undefined" ||
      import.meta.env.DEV ||
      window.location.pathname.startsWith("/admin") ||
      isShareRoutePathname(window.location.pathname)
    ) {
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

    clarityWindow.clarity("consentv2", {
      ad_Storage: "denied",
      analytics_Storage: "granted",
    });
    clarityWindow.clarity("start");

    if (document.getElementById(MICROSOFT_CLARITY_SCRIPT_ID)) {
      return disableMicrosoftClarity;
    }

    const script = document.createElement("script");
    script.id = MICROSOFT_CLARITY_SCRIPT_ID;
    script.async = true;
    script.src = `https://www.clarity.ms/tag/${MICROSOFT_CLARITY_TAG_ID}`;
    document.head.appendChild(script);

    return disableMicrosoftClarity;
  });

  return null;
}

export function WebProviders({
  children,
  queryClient,
  telemetryEnabled,
}: {
  children: React.ReactNode;
  queryClient: QueryClient;
  telemetryEnabled: boolean;
}) {
  return (
    <PostHogProvider enabled={telemetryEnabled}>
      <QueryClientProvider client={queryClient}>
        {children}
        <BrowserTelemetryRouteGuard
          key={telemetryEnabled ? "enabled" : "disabled"}
          enabled={telemetryEnabled}
        />
        {telemetryEnabled ? (
          <>
            <MicrosoftClarityScript />
            <GoogleAnalyticsScript />
          </>
        ) : (
          <PrivateAnalyticsGuard />
        )}
      </QueryClientProvider>
    </PostHogProvider>
  );
}

function BrowserTelemetryRouteGuard({ enabled }: { enabled: boolean }) {
  useMountEffect(() => {
    if (enabled) {
      bootstrapBrowserTelemetry();
    } else {
      stopBrowserTelemetry();
    }
  });

  return null;
}

function PrivateAnalyticsGuard() {
  useMountEffect(() => {
    setGoogleAnalyticsDisabled(true);
    disableMicrosoftClarity();
  });

  return null;
}

function setGoogleAnalyticsDisabled(disabled: boolean) {
  if (typeof window === "undefined") {
    return;
  }
  (window as unknown as Record<string, unknown>)[
    `ga-disable-${GOOGLE_ANALYTICS_ID}`
  ] = disabled;
}

function disableMicrosoftClarity() {
  if (typeof window === "undefined") {
    return;
  }

  const clarity = (window as ClarityWindow).clarity;
  clarity?.("consentv2", {
    ad_Storage: "denied",
    analytics_Storage: "denied",
  });
  clarity?.("stop");
}
