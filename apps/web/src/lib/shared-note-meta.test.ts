import assert from "node:assert/strict";
import test from "node:test";

import {
  getLinkShareHead,
  getPrivateShareHead,
  getPublicShareHead,
  privateShareHeaders,
} from "./shared-note-meta.ts";

const shareId = "00000000-0000-4000-8000-000000000001";
const previewToken = "a".repeat(64);

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

test("authorized link previews receive note-specific social metadata", () => {
  const head = getLinkShareHead(shareId, previewToken, {
    title: "Sprint planning",
    summary: "The team agreed on the next sprint's priorities.",
    participants: ["John Jeong", "Sungbin Jo"],
    meetingAt: "2026-08-06T00:00:00Z",
  });

  assert.ok(
    head.meta.some(
      (meta) =>
        "property" in meta &&
        meta.property === "og:title" &&
        meta.content === "Sprint planning",
    ),
  );
  assert.ok(
    head.meta.some(
      (meta) =>
        "property" in meta &&
        meta.property === "og:image" &&
        meta.content ===
          `https://anarlog.so/api/og/share/link/${shareId}?preview=${previewToken}`,
    ),
  );
  assert.ok(
    head.meta.some(
      (meta) =>
        "name" in meta &&
        meta.name === "robots" &&
        meta.content?.includes("noindex"),
    ),
  );
});

test("link previews fail closed without a valid preview result", () => {
  assert.deepEqual(
    getLinkShareHead(shareId, previewToken, null),
    getPrivateShareHead(),
  );
});

test("available public notes receive canonical indexable metadata", () => {
  const head = getPublicShareHead(
    "s_0123456789abcdef0123456789abcdef",
    {
      shareId,
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
    },
    {
      title: "Public note",
      summary: "A useful public summary",
      participants: ["John Jeong", "Sungbin Jo"],
      meetingAt: "2026-07-17T09:00:00Z",
    },
  );

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
  assert.ok(
    head.meta.some(
      (meta) =>
        meta.name === "description" &&
        meta.content === "John Jeong, Sungbin Jo · July 17, 2026",
    ),
  );
  assert.ok(
    head.meta.some(
      (meta) =>
        meta.property === "og:image" &&
        meta.content ===
          "https://anarlog.so/api/og/share/public/s_0123456789abcdef0123456789abcdef",
    ),
  );
  assert.ok(
    head.meta.some(
      (meta) =>
        meta.name === "twitter:card" && meta.content === "summary_large_image",
    ),
  );
});

test("unavailable public routes fail closed to private metadata", () => {
  assert.deepEqual(
    getPublicShareHead("s_0123456789abcdef0123456789abcdef", null),
    getPrivateShareHead(),
  );
});
