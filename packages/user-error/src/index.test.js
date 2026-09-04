import assert from "node:assert/strict";
import test from "node:test";

import { isUserError, isUserErrorEvent } from "./index.js";

test("recognizes provider quota and API key failures", () => {
  assert.equal(
    isUserError({
      error: {
        code: "insufficient_quota",
        message: "You exceeded your current quota",
      },
    }),
    true,
  );
  assert.equal(isUserError(new Error("Invalid API key")), true);
  assert.equal(isUserError(new Error("API key is invalid")), true);
  assert.equal(isUserError(new Error("Invalid APIKEY")), true);
  assert.equal(isUserError(new Error("socket hang up")), false);
});

test("matches the current event without inspecting breadcrumbs", () => {
  assert.equal(
    isUserErrorEvent({
      exception: {
        values: [{ type: "ProviderError", value: "quota exceeded" }],
      },
    }),
    true,
  );
  assert.equal(
    isUserErrorEvent({
      exception: { values: [{ type: "RouteError", value: "socket hang up" }] },
      breadcrumbs: [{ message: "quota exceeded" }],
    }),
    false,
  );
});

test("ignores stack source and built-in context text", () => {
  assert.equal(
    isUserErrorEvent({
      exception: {
        values: [
          {
            type: "TypeError",
            value: "Cannot read properties of undefined",
            stacktrace: {
              frames: [{ context_line: 'throw new Error("quota exceeded")' }],
            },
          },
        ],
      },
      contexts: {
        trace: { data: { request: "quota exceeded" } },
      },
    }),
    false,
  );
});

test("matches explicit tags and operation context", () => {
  assert.equal(
    isUserErrorEvent({ tags: { "provider.error": "insufficient_quota" } }),
    true,
  );
  assert.equal(
    isUserErrorEvent({
      contexts: {
        "anarlog.operation": { providerError: "invalid api key" },
      },
    }),
    true,
  );
});

test("handles cyclic error metadata", () => {
  /** @type {Record<string, unknown>} */
  const value = { message: "connection failed" };
  value.context = value;
  assert.equal(isUserError(value), false);
});
