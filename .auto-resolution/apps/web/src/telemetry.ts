import { HoneycombWebSDK } from "@honeycombio/opentelemetry-web";
import { getWebAutoInstrumentations } from "@opentelemetry/auto-instrumentations-web";

import { env } from "./env";
import { isShareRoutePathname } from "./lib/share-route-privacy";

declare global {
  interface Window {
    __hyprWebOtelSdk?: HoneycombWebSDK;
  }
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function normalizeUrl(value: string): string {
  return value.replace(/\/v1\/traces\/?$/, "").replace(/\/$/, "");
}

function buildUrlPrefixPattern(value: string): RegExp {
  return new RegExp(`^${escapeRegExp(normalizeUrl(value))}(?:$|[/?#])`);
}

function getOrigin(value: string | undefined): string | null {
  try {
    if (!value) {
      return null;
    }
    return new URL(value).origin;
  } catch {
    return null;
  }
}

function getCollectorTraceUrlPattern(endpoint: string): RegExp {
  return new RegExp(
    `^${escapeRegExp(`${normalizeUrl(endpoint)}/v1/traces`)}(?:$|[?#])`,
  );
}

function getIgnoredUrls(endpoint: string): RegExp[] {
  const ignoredUrls = [getCollectorTraceUrlPattern(endpoint)];

  if (env.VITE_SENTRY_DSN) {
    const sentryOrigin = getOrigin(env.VITE_SENTRY_DSN);
    if (sentryOrigin) {
      ignoredUrls.push(buildUrlPrefixPattern(sentryOrigin));
    }
  }

  ignoredUrls.push(buildUrlPrefixPattern(env.VITE_POSTHOG_HOST));
  ignoredUrls.push(
    buildUrlPrefixPattern(
      new URL("/shared-notes/", env.VITE_API_URL).toString(),
    ),
  );

  return ignoredUrls;
}

function getPropagationTargets(): string[] {
  if (typeof window === "undefined") {
    return [];
  }

  const targets = new Set<string>();

  for (const value of [env.VITE_API_URL, env.VITE_SUPABASE_URL]) {
    const origin = getOrigin(value);
    if (origin && origin !== window.location.origin) {
      targets.add(origin);
    }
  }

  return [...targets];
}

export function bootstrapBrowserTelemetry() {
  if (
    typeof window === "undefined" ||
    isShareRoutePathname(window.location.pathname)
  ) {
    return;
  }

  const endpoint = env.VITE_OTEL_EXPORTER_OTLP_ENDPOINT;

  if (!endpoint) {
    return;
  }

  if (window.__hyprWebOtelSdk) {
    return;
  }

  const ignoreUrls = getIgnoredUrls(endpoint);
  const propagateTraceHeaderCorsUrls = getPropagationTargets();

  const sdk = new HoneycombWebSDK({
    endpoint: normalizeUrl(endpoint),
    instrumentations: [
      getWebAutoInstrumentations({
        "@opentelemetry/instrumentation-fetch": {
          ignoreUrls,
          propagateTraceHeaderCorsUrls,
        },
        "@opentelemetry/instrumentation-xml-http-request": {
          ignoreUrls,
          propagateTraceHeaderCorsUrls,
        },
      }),
    ],
    resourceAttributes: {
      "deployment.environment": import.meta.env.DEV
        ? "development"
        : "production",
      "service.namespace": "hyprnote",
    },
    sampleRate: env.VITE_OTEL_SAMPLE_RATE,
    serviceName: "web",
    serviceVersion: env.VITE_APP_VERSION,
    skipOptionsValidation: true,
  });

  sdk.start();
  window.__hyprWebOtelSdk = sdk;
}

export function stopBrowserTelemetry() {
  if (typeof window === "undefined" || !window.__hyprWebOtelSdk) {
    return;
  }

  const sdk = window.__hyprWebOtelSdk;
  window.__hyprWebOtelSdk = undefined;
  void sdk.shutdown().catch(() => {});
}
