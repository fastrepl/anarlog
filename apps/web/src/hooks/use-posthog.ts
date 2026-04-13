import { usePostHog } from "@posthog/react";
import { useCallback } from "react";

import { usePostHogReady } from "@/providers/posthog";

export { usePostHog };

/**
 * Hook for type-safe PostHog event tracking.
 * All callbacks are stable references that update when PostHog initializes,
 * so mount-time useEffects depending on them will re-run after init.
 */
export function useAnalytics() {
  const posthog = usePostHog();
  const analyticsReady = usePostHogReady();

  const track = useCallback(
    (eventName: string, properties?: Record<string, any>) => {
      if (!analyticsReady || !posthog) {
        return;
      }
      posthog.capture(eventName, properties);
    },
    [posthog, analyticsReady],
  );

  const identify = useCallback(
    (userId: string, properties?: Record<string, any>) => {
      if (!analyticsReady || !posthog) {
        return;
      }
      posthog.identify(userId, properties);
    },
    [posthog, analyticsReady],
  );

  const reset = useCallback(() => {
    if (!analyticsReady || !posthog) {
      return;
    }
    posthog.reset();
  }, [posthog, analyticsReady]);

  return {
    track,
    identify,
    reset,
    posthog,
    analyticsReady,
  };
}
