import { PostHogProvider as PostHogReactProvider } from "@posthog/react";
import posthog from "posthog-js";
import { createContext, useContext, useEffect, useRef, useState } from "react";

import { env } from "../env";

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
  const [isInitialized, setIsInitialized] = useState(false);

  useEffect(() => {
    if (
      typeof window === "undefined" ||
      !enabled ||
      !env.VITE_POSTHOG_API_KEY ||
      isDev
    ) {
      setIsInitialized(false);
      return;
    }

    if (!didInitRef.current) {
      posthog.init(env.VITE_POSTHOG_API_KEY, {
        api_host: env.VITE_POSTHOG_HOST,
        autocapture: true,
        capture_pageview: true,
      });
      didInitRef.current = true;
    }

    setIsInitialized(true);
  }, [enabled]);

  if (!env.VITE_POSTHOG_API_KEY || isDev) {
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
