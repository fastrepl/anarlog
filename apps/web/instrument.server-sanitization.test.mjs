import assert from "node:assert/strict";
import test from "node:test";

import { sanitizeServerErrorEvent } from "./instrument.server-sanitization.mjs";

test("removes private server stack details", () => {
  const event = sanitizeServerErrorEvent({
    tags: {
      "anarlog.operation": "session_save",
      "http.response.status_code": 503,
      "anarlog.note_id": "note-1",
    },
    exception: {
      values: [
        {
          type: "RouteError",
          value: "private note content",
          module: "/Users/alice/private.ts",
          mechanism: {
            type: "generic",
            handled: false,
            data: { token: "secret" },
          },
          stacktrace: {
            frames: [
              {
                filename: "/Users/alice/private.ts",
                function: "saveSession",
                lineno: 42,
                vars: { transcript: "private transcript" },
              },
            ],
          },
        },
      ],
    },
  });

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
  assert.deepEqual(event.tags, {
    "anarlog.operation": "session_save",
    "http.response.status_code": 503,
  });
});
