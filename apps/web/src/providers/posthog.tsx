import type { CaptureResult, PostHog } from "posthog-js";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

import { env } from "../env";
import {
  sanitizeAnalyticsEventName,
  sanitizeAnalyticsProperties,
} from "../lib/analytics-sanitization";
import { isTelemetryPrivateLocation } from "../lib/auth-route-privacy";
import { hasGlobalPrivacyControl } from "../lib/global-privacy-control";
import { runWhenIdle } from "../lib/run-when-idle";

const isDev = import.meta.env.DEV;
const POSTHOG_URL_PROPERTIES = [
  "$current_url",
  "$initial_current_url",
  "$initial_referrer",
  "$referrer",
] as const;
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function normalizePath(pathname: string) {
  return pathname
    .split("/")
    .map((segment) => {
      let decoded = segment;
      try {
        decoded = decodeURIComponent(segment);
      } catch {
        return ":id";
      }
      return UUID_PATTERN.test(decoded) ||
        EMAIL_PATTERN.test(decoded) ||
        /^\d{6,}$/.test(decoded) ||
        decoded.length > 32
        ? ":id"
        : segment;
    })
    .join("/");
}

function sanitizePostHogEvent(event: CaptureResult | null) {
  if (!event) return null;
  const properties = { ...event.properties };
  for (const key of POSTHOG_URL_PROPERTIES) {
    const value = properties[key];
    if (typeof value !== "string") continue;
    try {
      const url = new URL(value, window.location.origin);
      properties[key] = `${url.origin}${normalizePath(url.pathname)}`;
    } catch {
      delete properties[key];
    }
  }
  if (typeof properties.$pathname === "string") {
    properties.$pathname = normalizePath(
      properties.$pathname.split(/[?#]/, 1)[0],
    );
  }
  return {
    ...event,
    event: sanitizeAnalyticsEventName(event.event),
    properties: sanitizeAnalyticsProperties(properties),
  };
}

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
          mask_all_element_attributes: true,
          mask_all_text: true,
          session_recording: {
            maskAllInputs: true,
            maskTextSelector: "*",
          },
          before_send: (event) =>
            isTelemetryPrivateLocation(
              window.location.pathname,
              window.location.search,
            )
              ? null
              : sanitizePostHogEvent(event),
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
