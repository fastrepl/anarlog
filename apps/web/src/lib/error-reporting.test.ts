import assert from "node:assert/strict";
import test from "node:test";

import {
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
        },
      ],
    },
    extra: { transcript: "private transcript" },
  });

  assert.equal(event.message, undefined);
  assert.equal(event.logentry, undefined);
  assert.equal(event.extra, undefined);
  assert.deepEqual(event.exception?.values, [
    {
      type: "RouteError",
      value: "RouteError captured",
      mechanism: { type: "generic", handled: false },
    },
  ]);
});
