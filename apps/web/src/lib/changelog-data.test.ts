import assert from "node:assert/strict";
import test from "node:test";

import { createChangelogEntries } from "./changelog-data.ts";

test("the combined feed orders by publication time and preserves identical versions in different streams", () => {
  const raw =
    '---\ndate: "2026-12-01"\nsummary: "Release summary"\n---\nRelease body';
  const entries = createChangelogEntries({
    "/content/desktop/1.4.23.md": {
      raw,
      publication: {
        publishedAt: "2026-09-08T11:12:17Z",
        channel: "stable",
        availability: [],
      },
    },
    "/content/mobile/1.4.23.md": {
      raw,
      publication: {
        publishedAt: "2026-09-09T11:12:17Z",
        channel: "beta",
        availability: [{ platform: "ios", destination: "testflight" }],
      },
    },
  });
  assert.deepEqual(
    entries.map(({ stream, version, date }) => ({ stream, version, date })),
    [
      { stream: "mobile", version: "1.4.23", date: "2026-09-09" },
      { stream: "desktop", version: "1.4.23", date: "2026-09-08" },
    ],
  );
  assert.equal(entries[0].summary, "Release summary");
  assert.equal(entries[0].content, "Release body");
  assert.deepEqual(entries[0].availability, [
    { platform: "ios", destination: "testflight" },
  ]);
});
