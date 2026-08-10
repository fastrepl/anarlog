import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  addIntegration: vi.fn(),
  emit: vi.fn(),
  getClient: vi.fn(),
  isDisabled: vi.fn(),
  listener: undefined as undefined | (() => void),
  listen: vi.fn(),
  publicStopReplay: vi.fn(),
  replayIntegration: vi.fn(),
  stopReplay: vi.fn(),
  withScope: vi.fn(),
}));

vi.mock("@sentry/react", () => ({
  addIntegration: mocks.addIntegration,
  captureException: vi.fn(),
  getClient: mocks.getClient,
  init: vi.fn(),
  replayIntegration: mocks.replayIntegration,
  setUser: vi.fn(),
  withScope: mocks.withScope,
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: mocks.emit,
  listen: mocks.listen,
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: { isDisabled: mocks.isDisabled },
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
  mocks.isDisabled.mockResolvedValue({ status: "ok", data: false });
  mocks.listen.mockImplementation(async (_event, listener) => {
    mocks.listener = listener;
    return vi.fn();
  });
  mocks.replayIntegration.mockReturnValue({ name: "Replay" });
  mocks.stopReplay.mockResolvedValue(undefined);
});

describe("normalizeOperationalError", () => {
  it("preserves useful fields from structured API failures", () => {
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
    ).toBe(
      "integration_connect failed (status=403, code=subscription_required, message=A Pro subscription is required)",
    );
  });

  it("keeps primitive failures and ignores arbitrary object data", () => {
    expect(
      normalizeOperationalError("connection refused", "sync").message,
    ).toBe("sync failed: connection refused");
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

    expect(event?.user).toEqual({ id: "user-1" });
    expect(event?.request).toEqual({
      method: "POST",
      url: "https://anarlog.so/note/123",
    });
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
        data: {
          from: "https://anarlog.so/",
          to: "https://anarlog.so/app/",
        },
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

describe("session replay consent", () => {
  it("broadcasts revocation and stops the local replay", async () => {
    vi.resetModules();
    const { disableSessionReplay, initializeErrorReporting } =
      await import("./error-reporting");

    initializeErrorReporting();
    await vi.waitFor(() => expect(mocks.addIntegration).toHaveBeenCalledOnce());

    disableSessionReplay();

    expect(mocks.stopReplay).toHaveBeenCalledWith({
      forceFlush: false,
      reason: "consent_revoked",
    });
    expect(mocks.publicStopReplay).not.toHaveBeenCalled();
    expect(mocks.emit).toHaveBeenCalledWith("anlg:session-replay-disabled");
  });

  it("stops replay when another webview revokes consent", async () => {
    vi.resetModules();
    const { initializeErrorReporting } = await import("./error-reporting");

    initializeErrorReporting();
    await vi.waitFor(() => expect(mocks.addIntegration).toHaveBeenCalledOnce());

    mocks.listener?.();

    expect(mocks.stopReplay).toHaveBeenCalledOnce();
  });

  it("does not attach replay when revocation races with consent loading", async () => {
    vi.resetModules();
    let resolveConsent!: (value: { status: "ok"; data: boolean }) => void;
    mocks.isDisabled.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveConsent = resolve;
      }),
    );
    const { initializeErrorReporting } = await import("./error-reporting");

    initializeErrorReporting();
    await vi.waitFor(() => expect(mocks.isDisabled).toHaveBeenCalledOnce());
    mocks.listener?.();
    resolveConsent({ status: "ok", data: false });

    await Promise.resolve();
    expect(mocks.addIntegration).not.toHaveBeenCalled();
  });
});
