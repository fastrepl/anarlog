import type { PostHog } from "posthog-js";
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
import { hasGlobalPrivacyControl } from "../lib/global-privacy-control";
import { runWhenIdle } from "../lib/run-when-idle";

const isDev = import.meta.env.DEV;

type PendingAnalyticsOperation = (client: PostHog) => void;

const PostHogContext = createContext<{
  analyticsReady: boolean;
  client: PostHog | null;
  runOrQueue: (operation: PendingAnalyticsOperation) => void;
}>({
  analyticsReady: false,
  client: null,
  runOrQueue: () => {},
});

export function usePostHogReady() {
  return useContext(PostHogContext).analyticsReady;
}

export function usePostHogClient() {
  return useContext(PostHogContext).client;
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
  const clientRef = useRef<PostHog | null>(null);
  const pendingOperationsRef = useRef<PendingAnalyticsOperation[]>([]);
  const [isInitialized, setIsInitialized] = useState(false);
  const globalPrivacyControl = hasGlobalPrivacyControl();
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
    const client = clientRef.current;
    if (analyticsStatusRef.current === "ready" && client) {
      operation(client);
    } else if (analyticsStatusRef.current === "pending") {
      pendingOperationsRef.current.push(operation);
    }
  }, []);

  useEffect(() => {
    const existingClient = clientRef.current;
    const apiKey = env.VITE_POSTHOG_API_KEY;

    if (!analyticsAvailable || !apiKey) {
      pendingOperationsRef.current = [];
      if (globalPrivacyControl && existingClient) {
        existingClient.opt_out_capturing();
      } else if (existingClient) {
        existingClient.set_config({
          autocapture: false,
          capture_pageview: false,
          disable_session_recording: true,
        });
        existingClient.stopSessionRecording();
        routeDisabledRef.current = true;
      }
      setIsInitialized(false);
      return;
    }

    let cancelled = false;

    const enableClient = (client: PostHog) => {
      if (cancelled) return;

      clientRef.current = client;
      if (!didInitRef.current) {
        client.init(apiKey, {
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
        client.set_config({
          autocapture: true,
          capture_pageview: true,
          disable_session_recording: false,
        });
        client.startSessionRecording();
        routeDisabledRef.current = false;
      }

      analyticsStatusRef.current = "ready";
      const pendingOperations = pendingOperationsRef.current.splice(0);
      for (const operation of pendingOperations) {
        operation(client);
      }
      setIsInitialized(true);
    };

    if (existingClient) {
      enableClient(existingClient);
      return () => {
        cancelled = true;
      };
    }

    const cancelIdle = runWhenIdle(() => {
      void import("posthog-js")
        .then(({ default: client }) => {
          enableClient(client);
        })
        .catch(() => {
          if (!cancelled) {
            analyticsStatusRef.current = "disabled";
            pendingOperationsRef.current = [];
          }
        });
    });

    return () => {
      cancelled = true;
      cancelIdle();
    };
  }, [analyticsAvailable, globalPrivacyControl]);

  return (
    <PostHogContext.Provider
      value={{
        analyticsReady,
        client: clientRef.current,
        runOrQueue,
      }}
    >
      {children}
    </PostHogContext.Provider>
  );
}
