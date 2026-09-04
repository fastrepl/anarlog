import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  addIntegration: vi.fn(),
  clearBreadcrumbs: vi.fn(),
  emit: vi.fn(),
  getClient: vi.fn(),
  isCrashReportingEnabled: vi.fn(),
  listener: undefined as
    | undefined
    | ((event: { payload: { enabled: boolean } }) => void),
  listen: vi.fn(),
  publicStopReplay: vi.fn(),
  replayIntegration: vi.fn(),
  setCrashReportingEnabled: vi.fn(),
  startReplay: vi.fn(),
  stopReplay: vi.fn(),
  withScope: vi.fn(),
}));

vi.mock("@sentry/react", () => ({
  addIntegration: mocks.addIntegration,
  captureException: vi.fn(),
  getClient: mocks.getClient,
  getCurrentScope: () => ({ clearBreadcrumbs: mocks.clearBreadcrumbs }),
  getReplay: () => ({ start: mocks.startReplay }),
  init: vi.fn(),
  replayIntegration: mocks.replayIntegration,
  setUser: vi.fn(),
  withScope: mocks.withScope,
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: mocks.emit,
  listen: mocks.listen,
}));

vi.mock("./types/tauri.gen", () => ({
  commands: {
    isCrashReportingEnabled: mocks.isCrashReportingEnabled,
    setCrashReportingEnabled: mocks.setCrashReportingEnabled,
  },
}));

vi.mock("./env", () => ({
  env: {
    VITE_APP_VERSION: "test",
    VITE_SENTRY_DSN: "https://public@example.com/1",
  },
}));

import {
  captureOperationalError,
  normalizeOperationalError,
  operationalErrorMetadata,
  sanitizeErrorEvent,
} from "./error-reporting";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.listener = undefined;
  mocks.emit.mockResolvedValue(undefined);
  mocks.getClient.mockReturnValue({
    getIntegrationByName: () => ({
      _replay: { stop: mocks.stopReplay },
      stop: mocks.publicStopReplay,
    }),
  });
  mocks.isCrashReportingEnabled.mockResolvedValue({
    status: "ok",
    data: true,
  });
  mocks.setCrashReportingEnabled.mockResolvedValue({
    status: "ok",
    data: null,
  });
  mocks.listen.mockImplementation(async (_event, listener) => {
    mocks.listener = listener;
    return vi.fn();
  });
  mocks.replayIntegration.mockReturnValue({ name: "Replay" });
  mocks.stopReplay.mockResolvedValue(undefined);
});

describe("normalizeOperationalError", () => {
  it("removes structured API failure messages", () => {
    expect(
      normalizeOperationalError(
        {
          status: 403,
          code: "subscription_required",
          message: "A Pro subscription is required",
          responseBody: "private response body",
        },
        "integration_connect",
      ).message,
    ).toBe("integration_connect failed");
  });

  it("removes primitive failures and arbitrary object data", () => {
    expect(
      normalizeOperationalError("connection refused", "sync").message,
    ).toBe("sync failed");
    expect(
      normalizeOperationalError({ transcript: "private transcript" }, "sync")
        .message,
    ).toBe("sync failed");
  });

  it("extracts only privacy-safe operational diagnostics", () => {
    expect(
      operationalErrorMetadata({
        name: "ApiError",
        code: "subscription_required",
        stage: "billing_check",
        statusCode: 403,
        message: "private@example.com",
      }),
    ).toEqual({
      type: "ApiError",
      code: "subscription_required",
      stage: "billing_check",
      status: 403,
    });
    expect(
      operationalErrorMetadata({
        code: "private email@example.com",
        stage: "billing check",
        status: 999,
      }),
    ).toEqual({
      type: "Error",
      code: undefined,
      stage: undefined,
      status: undefined,
    });
  });
});

describe("sanitizeErrorEvent", () => {
  it("keeps diagnostics while removing user and request data", () => {
    const event = sanitizeErrorEvent({
      type: undefined,
      user: {
        id: "user-1",
        email: "private@example.com",
        ip_address: "127.0.0.1",
      },
      request: {
        method: "POST",
        url: "https://anarlog.so/note/123?token=secret#selection",
        headers: { authorization: "Bearer secret" },
        data: { note: "private note" },
      },
      message: "private@example.com opened a private note",
      logentry: {
        message: "token=secret",
      },
      exception: {
        values: [
          {
            type: "RouteError",
            value: "private note content",
            mechanism: {
              type: "generic",
              handled: false,
              data: { token: "secret" },
            },
            module: "/Users/alice/private.ts",
            stacktrace: {
              frames: [
                {
                  filename: "https://anarlog.so/app?token=secret",
                  function: "saveSession",
                  lineno: 42,
                  vars: { transcript: "private transcript" },
                },
              ],
            },
          },
        ],
      },
      extra: { transcript: "private transcript" },
      breadcrumbs: [
        {
          category: "console",
          message: "private note",
          data: { arguments: ["private note"] },
        },
        {
          category: "navigation",
          data: {
            from: "https://anarlog.so/?token=secret",
            to: "https://anarlog.so/app/?share=secret",
          },
        },
      ],
    });

    expect(event?.user).toBeUndefined();
    expect(event?.request).toBeUndefined();
    expect(event?.extra).toBeUndefined();
    expect(event?.message).toBeUndefined();
    expect(event?.logentry).toBeUndefined();
    expect(event?.exception?.values).toEqual([
      {
        type: "RouteError",
        value: "RouteError captured",
        mechanism: {
          type: "generic",
          handled: false,
        },
        stacktrace: {
          frames: [
            {
              colno: undefined,
              filename: "source",
              function: "saveSession",
              in_app: undefined,
              lineno: 42,
            },
          ],
        },
      },
    ]);
    expect(event?.breadcrumbs).toEqual([
      {
        category: "console",
        level: undefined,
        timestamp: undefined,
        type: undefined,
      },
      {
        category: "navigation",
        level: undefined,
        timestamp: undefined,
        type: undefined,
      },
    ]);
  });
});

describe("user-caused failures", () => {
  const creditBalanceMessage =
    "Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.";

  it("drops account and credential failures before they reach Sentry", () => {
    expect(
      sanitizeErrorEvent({
        type: undefined,
        exception: {
          values: [{ type: "ProviderError", value: creditBalanceMessage }],
        },
      }),
    ).toBeNull();
    expect(
      sanitizeErrorEvent({
        type: undefined,
        extra: { "error.code": "insufficient_quota" },
      }),
    ).toBeNull();
  });

  it("keeps errors whose only match is an earlier breadcrumb", () => {
    expect(
      sanitizeErrorEvent({
        type: undefined,
        exception: { values: [{ type: "RouteError", value: "boom" }] },
        breadcrumbs: [{ category: "http", message: creditBalanceMessage }],
      }),
    ).not.toBeNull();
  });

  it("never captures them as operational errors", () => {
    captureOperationalError(new Error(creditBalanceMessage), {
      operation: "chat_completion",
    });

    expect(mocks.withScope).not.toHaveBeenCalled();

    captureOperationalError(new Error("socket hang up"), {
      operation: "chat_completion",
    });

    expect(mocks.withScope).toHaveBeenCalledOnce();
  });
});

describe("archived operational noise", () => {
  it("drops archived issue types before they reach Sentry", () => {
    expect(
      sanitizeErrorEvent({
        type: undefined,
        message:
          '[String("[runBatch] error handling batch response"), Object {}]',
      }),
    ).toBeNull();
    expect(
      sanitizeErrorEvent({
        type: undefined,
        exception: {
          values: [
            {
              type: "Error",
              value: "[listener] post-stop transcript repair failed",
            },
          ],
        },
      }),
    ).toBeNull();
    expect(
      sanitizeErrorEvent({
        type: undefined,
        logger: "tauri_plugin_tracing::ext",
        message: "native_error:tauri_plugin_tracing::ext:35",
      }),
    ).toBeNull();
  });

  it("never captures archived types as operational errors", () => {
    captureOperationalError(
      new Error("[runBatch] error handling batch response"),
      { operation: "batch_persist" },
    );

    expect(mocks.withScope).not.toHaveBeenCalled();
  });
});

describe("session replay consent", () => {
  it("broadcasts revocation and stops the local replay", async () => {
    vi.resetModules();
    const {
      initializeErrorReporting,
      setErrorReportingEnabled,
      setSessionReplayAnalyticsEnabled,
    } = await import("./error-reporting");

    initializeErrorReporting();
    setSessionReplayAnalyticsEnabled(true);
    await vi.waitFor(() => expect(mocks.addIntegration).toHaveBeenCalledOnce());

    await setErrorReportingEnabled(false);

    expect(mocks.stopReplay).toHaveBeenCalledWith({
      forceFlush: false,
      reason: "consent_revoked",
    });
    expect(mocks.publicStopReplay).not.toHaveBeenCalled();
    expect(mocks.setCrashReportingEnabled).toHaveBeenCalledWith(false);
    expect(mocks.emit).toHaveBeenCalledWith(
      "anlg:error-reporting-consent-changed",
      { enabled: false },
    );
  });

  it("stops replay when another webview revokes consent", async () => {
    vi.resetModules();
    const { initializeErrorReporting, setSessionReplayAnalyticsEnabled } =
      await import("./error-reporting");

    initializeErrorReporting();
    setSessionReplayAnalyticsEnabled(true);
    await vi.waitFor(() => expect(mocks.addIntegration).toHaveBeenCalledOnce());

    mocks.listener?.({ payload: { enabled: false } });

    expect(mocks.stopReplay).toHaveBeenCalledOnce();
  });

  it("restarts replay when Sentry consent is restored", async () => {
    vi.resetModules();
    const {
      initializeErrorReporting,
      setErrorReportingEnabled,
      setSessionReplayAnalyticsEnabled,
    } = await import("./error-reporting");

    initializeErrorReporting();
    setSessionReplayAnalyticsEnabled(true);
    await vi.waitFor(() => expect(mocks.addIntegration).toHaveBeenCalledOnce());

    await setErrorReportingEnabled(false);
    await setErrorReportingEnabled(true);

    expect(mocks.startReplay).toHaveBeenCalledOnce();
  });

  it("does not attach replay when revocation races with consent loading", async () => {
    vi.resetModules();
    let resolveConsent!: (value: { status: "ok"; data: boolean }) => void;
    mocks.isCrashReportingEnabled.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveConsent = resolve;
      }),
    );
    const { initializeErrorReporting, setSessionReplayAnalyticsEnabled } =
      await import("./error-reporting");

    initializeErrorReporting();
    setSessionReplayAnalyticsEnabled(true);
    await vi.waitFor(() =>
      expect(mocks.isCrashReportingEnabled).toHaveBeenCalledOnce(),
    );
    mocks.listener?.({ payload: { enabled: false } });
    resolveConsent({ status: "ok", data: true });

    await Promise.resolve();
    expect(mocks.addIntegration).not.toHaveBeenCalled();
  });
});
