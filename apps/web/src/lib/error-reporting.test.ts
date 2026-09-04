import assert from "node:assert/strict";
import test from "node:test";

import {
  createErrorEventFilter,
  operationalErrorMetadata,
  sanitizeErrorEvent,
} from "./error-reporting.ts";

test("extracts only privacy-safe operational diagnostics", () => {
  assert.deepEqual(
    operationalErrorMetadata({
      name: "ApiError",
      code: "subscription_required",
      stage: "billing_check",
      statusCode: 403,
      message: "private@example.com",
    }),
    {
      type: "ApiError",
      code: "subscription_required",
      stage: "billing_check",
      status: 403,
    },
  );
  assert.deepEqual(
    operationalErrorMetadata({
      code: "private email@example.com",
      stage: "billing check",
      status: 999,
    }),
    {
      type: "Error",
      code: undefined,
      stage: undefined,
      status: undefined,
    },
  );
});

test("removes private error content while preserving exception diagnostics", () => {
  const event = sanitizeErrorEvent({
    type: undefined,
    message: "private note",
    logentry: { message: "token=secret" },
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
    user: { id: "user-1", email: "private@example.com" },
    request: { method: "POST", url: "https://example.com/app/note-1?token=x" },
    contexts: { private: { type: "private", content: "patient note" } },
    tags: {
      "anarlog.operation": "session_save",
      "anarlog.note_id": "note-1",
    },
  });

  assert.equal(event.message, undefined);
  assert.equal(event.logentry, undefined);
  assert.equal(event.extra, undefined);
  assert.equal(event.user, undefined);
  assert.equal(event.request, undefined);
  assert.equal(event.contexts, undefined);
  assert.deepEqual(event.tags, { "anarlog.operation": "session_save" });
  assert.deepEqual(event.exception?.values, [
    {
      type: "RouteError",
      value: "RouteError captured",
      mechanism: { type: "generic", handled: false },
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
});

test("reports one grouped signal for repeated stackless promise rejections", () => {
  const filter = createErrorEventFilter();
  const event = () => ({
    type: undefined,
    exception: {
      values: [
        {
          type: "UnhandledRejection",
          value: "private rejection value",
          mechanism: {
            type: "auto.browser.global_handlers.onunhandledrejection",
            handled: false,
          },
        },
      ],
    },
  });

  const first = filter(event());
  assert.deepEqual(first?.fingerprint, [
    "web",
    "stackless-unhandled-rejection",
  ]);
  assert.equal(first?.tags?.["error.type"], "stackless_unhandled_rejection");
  assert.equal(filter(event()), null);
});

test("drops user-caused provider failures without inspecting breadcrumbs", () => {
  const filter = createErrorEventFilter();
  assert.equal(
    filter({
      type: undefined,
      exception: {
        values: [{ type: "ProviderError", value: "insufficient_quota" }],
      },
    }),
    null,
  );
  assert.notEqual(
    filter({
      type: undefined,
      exception: { values: [{ type: "RouteError", value: "socket hang up" }] },
      breadcrumbs: [{ message: "quota exceeded" }],
    }),
    null,
  );
});

test("does not rate-limit promise rejections with stack traces", () => {
  const filter = createErrorEventFilter();
  const event = () => ({
    type: undefined,
    exception: {
      values: [
        {
          type: "UnhandledRejection",
          value: "private rejection value",
          mechanism: {
            type: "auto.browser.global_handlers.onunhandledrejection",
            handled: false,
          },
          stacktrace: { frames: [{ filename: "app.ts" }] },
        },
      ],
    },
  });

  assert.notEqual(filter(event()), null);
  assert.notEqual(filter(event()), null);
});
