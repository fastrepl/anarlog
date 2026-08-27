import { useCallback } from "react";

import { env } from "@/env";
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
      runOrQueue((client) => {
        client.capture(eventName, {
          ...properties,
          surface: "web",
          analytics_schema_version: 1,
          app_version: env.VITE_APP_VERSION ?? "unknown",
        });
      });
    },
    [runOrQueue],
  );

  const identify = useCallback(
    (userId: string, properties?: Record<string, any>) => {
      runOrQueue((client) => client.identify(userId, properties));
    },
    [runOrQueue],
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
