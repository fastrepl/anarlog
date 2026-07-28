import { PostHogProvider as PostHogReactProvider } from "@posthog/react";
import posthog from "posthog-js";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

import { env } from "../env";
import { isTelemetryPrivateLocation } from "../lib/auth-route-privacy";

const isDev = import.meta.env.DEV;

type PendingAnalyticsOperation = (client: typeof posthog) => void;

const PostHogContext = createContext({
  analyticsReady: false,
  runOrQueue: (_operation: PendingAnalyticsOperation) => {},
});

export function usePostHogReady() {
  return useContext(PostHogContext).analyticsReady;
}

export function usePostHogOperation() {
  return useContext(PostHogContext).runOrQueue;
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
  const pendingOperationsRef = useRef<PendingAnalyticsOperation[]>([]);
  const [isInitialized, setIsInitialized] = useState(false);
  const globalPrivacyControl =
    typeof navigator !== "undefined" &&
    (navigator as Navigator & { globalPrivacyControl?: boolean })
      .globalPrivacyControl === true;
  const analyticsAvailable =
    typeof window !== "undefined" &&
    Boolean(env.VITE_POSTHOG_API_KEY) &&
    !isDev &&
    !globalPrivacyControl &&
    enabled &&
    !isTelemetryPrivateLocation(
      window.location.pathname,
      window.location.search,
    );
  const analyticsReady = analyticsAvailable && isInitialized;
  const analyticsStatusRef = useRef<"disabled" | "pending" | "ready">(
    analyticsAvailable ? "pending" : "disabled",
  );
  analyticsStatusRef.current = analyticsAvailable
    ? analyticsReady
      ? "ready"
      : "pending"
    : "disabled";

  const runOrQueue = useCallback((operation: PendingAnalyticsOperation) => {
    if (analyticsStatusRef.current === "ready") {
      operation(posthog);
    } else if (analyticsStatusRef.current === "pending") {
      pendingOperationsRef.current.push(operation);
    }
  }, []);

  useEffect(() => {
    if (!analyticsAvailable || !env.VITE_POSTHOG_API_KEY) {
      pendingOperationsRef.current = [];
      if (globalPrivacyControl && didInitRef.current) {
        posthog.opt_out_capturing();
      } else if (didInitRef.current) {
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
          isTelemetryPrivateLocation(
            window.location.pathname,
            window.location.search,
          )
            ? null
            : event,
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

    analyticsStatusRef.current = "ready";
    const pendingOperations = pendingOperationsRef.current.splice(0);
    for (const operation of pendingOperations) {
      operation(posthog);
    }
    setIsInitialized(true);
  }, [analyticsAvailable, globalPrivacyControl]);

  if (!enabled || !env.VITE_POSTHOG_API_KEY || isDev) {
    return (
      <PostHogContext.Provider value={{ analyticsReady, runOrQueue }}>
        {children}
      </PostHogContext.Provider>
    );
  }

  return (
    <PostHogContext.Provider value={{ analyticsReady, runOrQueue }}>
      <PostHogReactProvider client={posthog}>{children}</PostHogReactProvider>
    </PostHogContext.Provider>
  );
}
