import assert from "node:assert/strict";
import test from "node:test";

import { formatBlogDate } from "./blog-date.ts";

test("formats blog dates in UTC for stable server and client output", () => {
  assert.equal(formatBlogDate("2026-07-29", "long"), "July 29, 2026");
  assert.equal(formatBlogDate("2026-07-29", "short"), "Jul 29, 2026");
});

test("preserves invalid dates", () => {
  assert.equal(formatBlogDate("not-a-date", "long"), "not-a-date");
});
