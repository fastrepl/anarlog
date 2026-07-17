import { PostHogProvider as PostHogReactProvider } from "@posthog/react";
import posthog from "posthog-js";
import { createContext, useContext, useEffect, useRef, useState } from "react";

import { env } from "../env";
import { isShareRoutePathname } from "../lib/share-route-privacy";

const isDev = import.meta.env.DEV;

const PostHogReadyContext = createContext(false);

export function usePostHogReady() {
  return useContext(PostHogReadyContext);
}

export function PostHogProvider({
  children,
  enabled,
}: {
  children: React.ReactNode;
  enabled: boolean;
}) {
  const didInitRef = useRef(false);
  const routeDisabledRef = useRef(false);
  const [isInitialized, setIsInitialized] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined" || !env.VITE_POSTHOG_API_KEY || isDev) {
      setIsInitialized(false);
      return;
    }

    if (!enabled || isShareRoutePathname(window.location.pathname)) {
      if (didInitRef.current) {
        posthog.set_config({
          autocapture: false,
          capture_pageview: false,
          disable_session_recording: true,
        });
        posthog.stopSessionRecording();
        routeDisabledRef.current = true;
      }
      setIsInitialized(false);
      return;
    }

    if (!didInitRef.current) {
      posthog.init(env.VITE_POSTHOG_API_KEY, {
        api_host: env.VITE_POSTHOG_HOST,
        autocapture: true,
        capture_pageview: true,
        before_send: (event) =>
          isShareRoutePathname(window.location.pathname) ? null : event,
      });
      didInitRef.current = true;
    } else if (routeDisabledRef.current) {
      posthog.set_config({
        autocapture: true,
        capture_pageview: true,
        disable_session_recording: false,
      });
      posthog.startSessionRecording();
      routeDisabledRef.current = false;
    }

    setIsInitialized(true);
  }, [enabled]);

  if (!enabled || !env.VITE_POSTHOG_API_KEY || isDev) {
    return (
      <PostHogReadyContext.Provider value={isInitialized}>
        {children}
      </PostHogReadyContext.Provider>
    );
  }

  return (
    <PostHogReadyContext.Provider value={isInitialized}>
      <PostHogReactProvider client={posthog}>{children}</PostHogReactProvider>
    </PostHogReadyContext.Provider>
  );
}
