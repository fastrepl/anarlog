import * as Sentry from "@sentry/react";
import type { ErrorEvent, SeverityLevel } from "@sentry/react";
import { emit, listen } from "@tauri-apps/api/event";

import { isUserError, isUserErrorEvent } from "@anlg/user-error";

import { env } from "./env";
import { commands as desktopCommands } from "./types/tauri.gen";

type ErrorContextValue = null | boolean | number | string;
const SAFE_IDENTIFIER_RE = /^[a-zA-Z0-9_.:/-]{1,128}$/;
const ERROR_REPORTING_CONSENT_EVENT = "anlg:error-reporting-consent-changed";
let errorReportingEnabled = false;
let errorReportingConsentRevision = 0;

// Archived Sentry issue types. Local error logs stay; Sentry should not reopen
// or bill for issues that were already solved. Keep in sync with
// `IGNORED_ERROR_MARKERS` in crates/user-error/src/lib.rs.
const IGNORED_ERROR_MARKERS = [
  "[runbatch]",
  "post-stop transcript repair failed",
  "[audio-retention]",
  "batch transcription failed",
  "connect_async_failed",
  "acquired connection, but time to acquire exceeded",
  "slow statement",
  "samples_dropped",
  "mic_samples_dropped",
  "zoom_mic_usage_check_failed",
  "e2ee recovery key setup is required",
  "couldn't find callback id",
  "[sessionpersister]",
  "update_check_failed",
  "failed to check for updates",
  "failed_to_check_for_updates",
  "listen_ws_connect_failed",
  "listener_retry_failed",
  "failed to fetch remote connection ids",
];

function serializeForUserErrorMatch(value: unknown): string {
  if (typeof value === "string") return value;
  if (value instanceof Error) {
    return `${value.name}: ${value.message} ${serializeForUserErrorMatch({ ...value })}`;
  }

  try {
    return JSON.stringify(value) ?? "";
  } catch {
    return "";
  }
}

function textMatchesMarkers(value: unknown, markers: readonly string[]) {
  const text = serializeForUserErrorMatch(value).toLowerCase();
  return markers.some((marker) => text.includes(marker));
}

export function isIgnoredError(value: unknown): boolean {
  const text = serializeForUserErrorMatch(value);
  return (
    text.startsWith("[String(") ||
    text.startsWith("[Object {") ||
    textMatchesMarkers(text, IGNORED_ERROR_MARKERS)
  );
}

// Breadcrumbs are deliberately excluded: they outlive the failure that produced
// them and would suppress later, unrelated errors.
function isDroppedErrorEvent(event: ErrorEvent): boolean {
  const logger = typeof event.logger === "string" ? event.logger : undefined;
  if (
    logger &&
    (logger.startsWith("tauri_plugin_tracing") ||
      logger === "anarlog.webview.console" ||
      logger === "hyprnote.webview.console")
  ) {
    return true;
  }

  return (
    isUserErrorEvent(event) ||
    [event.message, event.logentry, event.exception, event.extra].some(
      isIgnoredError,
    )
  );
}

function safeIdentifier(value: unknown): string | undefined {
  return typeof value === "string" && SAFE_IDENTIFIER_RE.test(value)
    ? value
    : undefined;
}

export function operationalErrorMetadata(error: unknown) {
  if (!error || typeof error !== "object") {
    return { type: "Error" };
  }

  const record = error as Record<string, unknown>;
  const statusValue = record.status ?? record.statusCode;
  const status =
    typeof statusValue === "number" &&
    Number.isInteger(statusValue) &&
    statusValue >= 100 &&
    statusValue <= 599
      ? statusValue
      : undefined;

  return {
    type: safeIdentifier(record.name) ?? "Error",
    code: safeIdentifier(record.code),
    stage: safeIdentifier(record.stage),
    status,
  };
}

export function normalizeOperationalError(
  error: unknown,
  operation: string,
): Error {
  if (error instanceof Error) return error;

  if (
    typeof error === "string" ||
    typeof error === "number" ||
    typeof error === "boolean"
  ) {
    return new Error(`${operation} failed: ${String(error).slice(0, 256)}`);
  }

  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    const details = ["status", "statusCode", "code", "message"]
      .flatMap((key) => {
        const value = record[key];
        return typeof value === "string" ||
          typeof value === "number" ||
          typeof value === "boolean"
          ? [`${key}=${String(value).slice(0, 256)}`]
          : [];
      })
      .join(", ");

    if (details) {
      return new Error(`${operation} failed (${details})`);
    }
  }

  return new Error(`${operation} failed`);
}

function sanitizeUrl(value: string | undefined) {
  if (!value) return value;

  try {
    const url = new URL(value);
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return value.split(/[?#]/, 1)[0];
  }
}

export function sanitizeErrorEvent(event: ErrorEvent): ErrorEvent | null {
  if (isDroppedErrorEvent(event)) return null;

  if (event.user) {
    event.user = event.user.id ? { id: event.user.id } : undefined;
  }

  if (event.request) {
    event.request = {
      method: event.request.method,
      url: sanitizeUrl(event.request.url),
    };
  }

  delete event.extra;
  delete event.message;
  delete event.logentry;
  if (event.exception?.values) {
    event.exception.values = event.exception.values.map((exception) => {
      const sanitized = {
        ...exception,
        value: exception.type ? `${exception.type} captured` : "Error captured",
      };
      if (sanitized.mechanism) {
        delete sanitized.mechanism.data;
      }
      return sanitized;
    });
  }
  event.breadcrumbs = event.breadcrumbs?.map((breadcrumb) => ({
    category: breadcrumb.category,
    level: breadcrumb.level,
    timestamp: breadcrumb.timestamp,
    type: breadcrumb.type,
    ...(breadcrumb.category === "navigation"
      ? {
          data: {
            from: sanitizeUrl(String(breadcrumb.data?.from ?? "")),
            to: sanitizeUrl(String(breadcrumb.data?.to ?? "")),
          },
        }
      : {}),
  }));

  return event;
}

export function initializeErrorReporting() {
  if (!env.VITE_SENTRY_DSN) return;

  Sentry.init({
    dsn: env.VITE_SENTRY_DSN,
    release: env.VITE_APP_VERSION
      ? `anarlog-desktop@${env.VITE_APP_VERSION}`
      : undefined,
    environment: import.meta.env.MODE,
    sendDefaultPii: false,
    tracePropagationTargets: [],
    beforeSend: (event) =>
      errorReportingEnabled ? sanitizeErrorEvent(event) : null,
    replaysSessionSampleRate: 0.1,
    replaysOnErrorSampleRate: 1.0,
    initialScope: {
      tags: {
        "service.name": "desktop",
        "service.namespace": "anarlog",
        "anarlog.surface": "desktop",
      },
    },
  });

  void initializeErrorReportingConsent();
}

let sessionReplayInstalled = false;
let sessionReplayRunning = false;

async function initializeErrorReportingConsent() {
  try {
    await listen<{ enabled: boolean }>(
      ERROR_REPORTING_CONSENT_EVENT,
      ({ payload }) => {
        errorReportingConsentRevision += 1;
        applyErrorReportingConsent(payload.enabled);
      },
    );
    const consentRevision = errorReportingConsentRevision;
    const enabled = await desktopCommands.isCrashReportingEnabled();
    if (
      enabled.status === "ok" &&
      consentRevision === errorReportingConsentRevision
    ) {
      applyErrorReportingConsent(enabled.data);
    }
  } catch {
    // Without a readable consent state, keep error reporting off.
  }
}

function applyErrorReportingConsent(enabled: boolean) {
  errorReportingEnabled = enabled;
  Sentry.getCurrentScope().clearBreadcrumbs();

  if (enabled) {
    startSessionReplay();
  } else {
    stopSessionReplay();
  }
}

function startSessionReplay() {
  if (sessionReplayRunning) return;

  if (sessionReplayInstalled) {
    Sentry.getReplay()?.start();
    sessionReplayRunning = true;
    return;
  }

  Sentry.addIntegration(
    Sentry.replayIntegration({
      blockAllMedia: true,
      maskAllText: true,
    }),
  );
  sessionReplayInstalled = true;
  sessionReplayRunning = true;
}

function stopSessionReplay() {
  if (!sessionReplayRunning) return;

  sessionReplayRunning = false;
  const replay = Sentry.getClient()?.getIntegrationByName?.("Replay") as
    | {
        _replay?: {
          stop?: (options: {
            forceFlush: boolean;
            reason: string;
          }) => Promise<void>;
        };
      }
    | undefined;
  // The public stop() force-flushes session recordings, so consent revocation
  // must stop the internal controller without sending its buffered segment.
  void replay?._replay
    ?.stop?.({ forceFlush: false, reason: "consent_revoked" })
    .catch(() => {});
}

export async function setErrorReportingEnabled(enabled: boolean) {
  const result = await desktopCommands.setCrashReportingEnabled(enabled);
  if (result.status === "error") {
    throw new Error(result.error);
  }

  errorReportingConsentRevision += 1;
  applyErrorReportingConsent(enabled);
  await emit(ERROR_REPORTING_CONSENT_EVENT, { enabled });
}

export function captureOperationalError(
  error: unknown,
  {
    operation,
    level = "error",
    tags,
    context,
  }: {
    operation: string;
    level?: SeverityLevel;
    tags?: Record<string, ErrorContextValue>;
    context?: Record<string, ErrorContextValue>;
  },
) {
  if (isUserError(error) || isIgnoredError(error)) return;

  const metadata = operationalErrorMetadata(error);
  const exception = normalizeOperationalError(error, operation);

  return Sentry.withScope((scope) => {
    scope.setLevel(level);
    scope.setTag("anarlog.operation", operation);
    scope.setTag("error.type", metadata.type);
    if (metadata.code) scope.setTag("error.code", metadata.code);
    if (metadata.stage) {
      scope.setTag("anarlog.error.stage", metadata.stage);
    }
    if (metadata.status) {
      scope.setTag("http.response.status_code", metadata.status);
    }
    for (const [key, value] of Object.entries(tags ?? {})) {
      if (value !== null) {
        scope.setTag(`anarlog.${key}`, value);
      }
    }
    if (context) {
      scope.setContext("anarlog.operation", context);
    }
    return Sentry.captureException(exception);
  });
}

export function setErrorReportingUser(userId: string | null) {
  Sentry.setUser(userId ? { id: userId } : null);
}
