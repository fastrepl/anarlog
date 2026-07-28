import assert from "node:assert/strict";
import test from "node:test";

import {
  normalizeOperationalError,
  operationalErrorMetadata,
  sanitizeMobileErrorEvent,
} from "./error-sanitization.ts";

test("removes user content while preserving safe diagnostics", () => {
  const event = sanitizeMobileErrorEvent({
    platform: "javascript",
    tags: { "hyprnote.operation": "session_save" },
    user: {
      id: "user-1",
      email: "private@example.com",
      ip_address: "127.0.0.1",
    },
    request: {
      method: "POST",
      url: "https://api.anarlog.so/stt/listen?token=secret#selection",
      headers: { authorization: "Bearer secret" },
      data: { transcript: "private transcript" },
    },
    extra: { note: "private note" },
    message: "private note",
    logentry: { message: "private note" },
    exception: {
      values: [
        {
          type: "Error",
          value: "private note",
          mechanism: { data: { token: "secret" } },
        },
      ],
    },
    breadcrumbs: [
      {
        category: "hyprnote.operation",
        message: "auth_session_restore",
        data: {
          outcome: "persisted_fallback",
          email: "private@example.com",
        },
      },
      {
        category: "console",
        message: "private note",
        data: { arguments: ["private note"] },
      },
      {
        category: "navigation",
        data: {
          from: "anarlog://note/123?token=secret",
          to: "https://anarlog.so/app/?share=secret",
        },
      },
    ],
  });

  assert.deepEqual(event.user, { id: "user-1" });
  assert.deepEqual(event.request, {
    method: "POST",
    url: "https://api.anarlog.so/stt/listen",
  });
  assert.equal(event.extra, undefined);
  assert.equal(event.message, "session_save failed");
  assert.equal(event.logentry, undefined);
  assert.equal(event.exception?.values?.[0]?.value, "session_save failed");
  assert.equal(event.exception?.values?.[0]?.mechanism?.data, undefined);
  assert.deepEqual(event.breadcrumbs, [
    {
      category: "hyprnote.operation",
      level: undefined,
      message: "auth_session_restore",
      timestamp: undefined,
      type: undefined,
      data: {
        outcome: "persisted_fallback",
      },
    },
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
        from: "anarlog://note/123",
        to: "https://anarlog.so/app/",
      },
    },
  ]);
});

test("normalizes messages but retains stack frames and safe error fields", () => {
  const source = Object.assign(new Error("private@example.com token=secret"), {
    code: "refresh_token_not_found",
    stage: "auth_bootstrap",
    status: 401,
  });
  source.stack = [
    "Error: private@example.com token=secret",
    "    at restoreSession (auth.ts:42:3)",
  ].join("\n");

  const normalized = normalizeOperationalError(source, "auth_session_restore");

  assert.equal(normalized.message, "auth_session_restore failed");
  assert.equal(normalized.stack?.includes("private@example.com"), false);
  assert.equal(normalized.stack?.includes("auth.ts:42:3"), true);
  assert.deepEqual(operationalErrorMetadata(source), {
    type: "Error",
    code: "refresh_token_not_found",
    stage: "auth_bootstrap",
    status: 401,
  });
});
