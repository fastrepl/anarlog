import assert from "node:assert/strict";
import test from "node:test";

import {
  getPrivateShareHead,
  getPublicShareHead,
  privateShareHeaders,
} from "./shared-note-meta.ts";

test("private share metadata disables indexing, referrers, and AI indexing", () => {
  const head = getPrivateShareHead();
  assert.deepEqual(privateShareHeaders, {
    "Cache-Control": "private, no-store",
    "Referrer-Policy": "no-referrer",
    "X-Robots-Tag": "noindex, nofollow, noarchive, nosnippet",
  });
  assert.ok(
    head.meta.some(
      (meta) => meta.name === "robots" && meta.content.includes("noindex"),
    ),
  );
  assert.ok(
    head.meta.some(
      (meta) => meta.name === "ai-content" && meta.content === "private",
    ),
  );
});

test("available public notes receive canonical indexable metadata", () => {
  const head = getPublicShareHead("s_0123456789abcdef0123456789abcdef", {
    shareId: "00000000-0000-4000-8000-000000000001",
    schemaVersion: 1,
    contentRevision: 1,
    title: "Public note",
    body: {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "A useful public summary" }],
        },
      ],
    },
    attachments: [],
    publishedAt: "2026-07-17T12:00:00Z",
  });

  assert.ok("links" in head);
  if (!("links" in head)) {
    throw new Error("expected public metadata");
  }
  assert.deepEqual(head.links, [
    {
      rel: "canonical",
      href: "https://anarlog.so/share/public/s_0123456789abcdef0123456789abcdef/",
    },
  ]);
  assert.ok(
    head.meta.some(
      (meta) => meta.name === "robots" && meta.content === "index, follow",
    ),
  );
});

test("unavailable public routes fail closed to private metadata", () => {
  assert.deepEqual(
    getPublicShareHead("s_0123456789abcdef0123456789abcdef", null),
    getPrivateShareHead(),
  );
});
