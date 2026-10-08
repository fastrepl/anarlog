import { useCallback } from "react";

import { env } from "@/env";
import {
  sanitizeAnalyticsEventName,
  sanitizeAnalyticsProperties,
} from "@/lib/analytics-sanitization";
import { isTelemetryPrivateLocation } from "@/lib/auth-route-privacy";
import { hasGlobalPrivacyControl } from "@/lib/global-privacy-control";
import {
  usePostHogClient,
  usePostHogOperation,
  usePostHogReady,
} from "@/providers/posthog";

/**
 * Hook for type-safe PostHog event tracking.
 * All callbacks are stable references that use the latest readiness state.
 */
export function useAnalytics() {
  const posthog = usePostHogClient();
  const analyticsReady = usePostHogReady();
  const runOrQueue = usePostHogOperation();

  const track = useCallback(
    (eventName: string, properties?: Record<string, any>) => {
      if (eventName === "download_clicked" && typeof window !== "undefined") {
        const analyticsWindow = window as Window & {
          gtag?: (...args: unknown[]) => void;
        };
        if (
          !import.meta.env.DEV &&
          !hasGlobalPrivacyControl() &&
          !window.location.pathname.startsWith("/admin") &&
          !isTelemetryPrivateLocation(
            window.location.pathname,
            window.location.search,
          )
        ) {
          analyticsWindow.gtag?.("event", "conversion", {
            send_to: "AW-18481972229/i7OTCMKHiJUdEIWI8uxE",
          });
        }
      }
      runOrQueue((client) => {
        client.capture(sanitizeAnalyticsEventName(eventName), {
          ...sanitizeAnalyticsProperties(properties ?? {}),
          surface: "web",
          analytics_schema_version: 1,
          app_version: env.VITE_APP_VERSION ?? "unknown",
        });
      });
    },
    [runOrQueue],
  );

  const identify = useCallback(
    (_userId: string, _properties?: Record<string, any>) => undefined,
    [],
  );

  const reset = useCallback(() => {
    runOrQueue((client) => client.reset());
  }, [runOrQueue]);

  return {
    track,
    identify,
    reset,
    posthog,
    analyticsReady,
  };
}
