import { describe, expect, it } from "vitest";

import {
  normalizeOperationalError,
  operationalErrorMetadata,
  sanitizeErrorEvent,
} from "./error-reporting";

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

    expect(event.user).toEqual({ id: "user-1" });
    expect(event.request).toEqual({
      method: "POST",
      url: "https://anarlog.so/note/123",
    });
    expect(event.extra).toBeUndefined();
    expect(event.message).toBeUndefined();
    expect(event.logentry).toBeUndefined();
    expect(event.exception?.values).toEqual([
      {
        type: "RouteError",
        value: "RouteError captured",
        mechanism: {
          type: "generic",
          handled: false,
        },
      },
    ]);
    expect(event.breadcrumbs).toEqual([
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
